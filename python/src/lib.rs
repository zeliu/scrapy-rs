use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use futures::executor::block_on;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString};
use scrapy_rs_core::error::Result as RsResult;
use scrapy_rs_core::item::{DynamicItem, Item};
use scrapy_rs_core::request::Request;
use scrapy_rs_core::response::Response;
use scrapy_rs_core::spider::{BasicSpider, ParseOutput, Spider};
use scrapy_rs_downloader::{Downloader, DownloaderConfig, HttpDownloader};
use scrapy_rs_engine::{Engine, EngineConfig, EngineStats};
use scrapy_rs_middleware::{
    ChainedRequestMiddleware, ChainedResponseMiddleware, DefaultHeadersMiddleware,
    RequestMiddleware, ResponseLoggerMiddleware, ResponseMiddleware, RetryMiddleware,
};
use scrapy_rs_pipeline::{PipelineType, JsonFilePipeline, LogPipeline, Pipeline};
use scrapy_rs_scheduler::{MemoryScheduler, Scheduler};
use tokio::runtime::Runtime;
use url::Url;
use ::scrapy_rs::settings::{Settings as RsSettings, SettingsError};
use ::scrapy_rs::config_adapters::{
    create_spider_from_settings,
    create_downloader_from_settings,
    create_scheduler_from_settings,
    engine_config_from_settings
};


/// Python module for scrapy_rs
#[pymodule]
fn scrapy_rs(_py: Python, m: &PyModule) -> PyResult<()> {
    // Initialize logger
    env_logger::init();
    
    // Add classes
    m.add_class::<PyRequest>()?;
    m.add_class::<PyResponse>()?;
    m.add_class::<PyItem>()?;
    m.add_class::<PySpider>()?;
    m.add_class::<PyEngine>()?;
    m.add_class::<PyEngineStats>()?;
    m.add_class::<PySettings>()?;
    m.add_class::<PyDownloaderConfig>()?;
    m.add_class::<PyDownloader>()?;
    m.add_class::<PyScheduler>()?;
    
    Ok(())
}

/// Convert a Python error to a string
fn py_err_to_string(err: PyErr) -> String {
    format!("{}", err)
}

/// Convert a Rust error to a Python error
fn rs_err_to_py_err(err: scrapy_rs_core::error::Error) -> PyErr {
    PyRuntimeError::new_err(format!("{}", err))
}

/// Convert a Python dictionary to a Rust HashMap
fn py_dict_to_hashmap(dict: &PyDict) -> PyResult<HashMap<String, String>> {
    let mut map = HashMap::new();
    
    for (key, value) in dict.iter() {
        let key = key.extract::<String>()?;
        let value = value.extract::<String>()?;
        map.insert(key, value);
    }
    
    Ok(map)
}

/// Convert a Python dictionary to a Rust serde_json::Value
fn py_dict_to_json(dict: &PyDict) -> PyResult<serde_json::Value> {
    let mut map = serde_json::Map::new();
    
    for (key, value) in dict.iter() {
        let key = key.extract::<String>()?;
        let value = py_to_json_value(value)?;
        map.insert(key, value);
    }
    
    Ok(serde_json::Value::Object(map))
}

/// Convert a Python value to a Rust serde_json::Value
fn py_to_json_value(value: &PyAny) -> PyResult<serde_json::Value> {
    if value.is_instance_of::<PyDict>() {
        py_dict_to_json(value.downcast::<PyDict>()?)
    } else if value.is_instance_of::<PyList>() {
        let list = value.downcast::<PyList>()?;
        let mut values = Vec::new();
        
        for item in list.iter() {
            values.push(py_to_json_value(item)?);
        }
        
        Ok(serde_json::Value::Array(values))
    } else if value.is_instance_of::<PyString>() {
        Ok(serde_json::Value::String(value.extract::<String>()?))
    } else if let Ok(val) = value.extract::<i64>() {
        Ok(serde_json::Value::Number(serde_json::Number::from(val)))
    } else if let Ok(val) = value.extract::<f64>() {
        // Handle float values
        match serde_json::Number::from_f64(val) {
            Some(num) => Ok(serde_json::Value::Number(num)),
            None => Err(PyValueError::new_err(format!("Invalid float value: {}", val))),
        }
    } else if let Ok(val) = value.extract::<bool>() {
        Ok(serde_json::Value::Bool(val))
    } else if value.is_none() {
        Ok(serde_json::Value::Null)
    } else {
        Err(PyValueError::new_err(format!("Unsupported Python type: {}", value)))
    }
}

/// Convert a Rust serde_json::Value to a Python object
fn json_to_py(py: Python, value: &serde_json::Value) -> PyResult<PyObject> {
    match value {
        serde_json::Value::Null => Ok(py.None()),
        serde_json::Value::Bool(b) => Ok(b.to_object(py)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i.to_object(py))
            } else if let Some(f) = n.as_f64() {
                Ok(f.to_object(py))
            } else {
                Err(PyValueError::new_err(format!("Unsupported JSON number: {}", n)))
            }
        }
        serde_json::Value::String(s) => Ok(s.to_object(py)),
        serde_json::Value::Array(arr) => {
            let list = PyList::empty(py);
            for item in arr {
                list.append(json_to_py(py, item)?)?;
            }
            Ok(list.to_object(py))
        }
        serde_json::Value::Object(obj) => {
            let dict = PyDict::new(py);
            for (key, value) in obj {
                dict.set_item(key, json_to_py(py, value)?)?;
            }
            Ok(dict.to_object(py))
        }
    }
}

/// Python wrapper for Request
#[pyclass]
struct PyRequest {
    inner: Request,
}

#[pymethods]
impl PyRequest {
    #[new]
    fn new(
        url: &str, 
        method: Option<&str>, 
        headers: Option<&PyDict>, 
        body: Option<&[u8]>,
        cookies: Option<&PyDict>,
        dont_filter: Option<bool>,
        timeout: Option<f64>,
        encoding: Option<&str>,
        flags: Option<Vec<String>>,
        proxy: Option<&str>
    ) -> PyResult<Self> {
        // Parse the URL
        let url = Url::parse(url).map_err(|e| {
            PyValueError::new_err(format!("Invalid URL: {}", e))
        })?;
        
        // Create a request with the given method
        let mut request = match method {
            Some("POST") => {
                if let Some(body_data) = body {
                    Request::post(url.as_str(), body_data.to_vec())
                        .map_err(rs_err_to_py_err)?
                } else {
                    return Err(PyValueError::new_err("POST request requires a body"));
                }
            }
            Some("PUT") => {
                if let Some(body_data) = body {
                    let mut req = Request::get(url.as_str()).map_err(rs_err_to_py_err)?;
                    req.method = scrapy_rs_core::request::Method::PUT;
                    req.body = Some(body_data.to_vec());
                    req
                } else {
                    return Err(PyValueError::new_err("PUT request requires a body"));
                }
            }
            Some("DELETE") => {
                let mut req = Request::get(url.as_str()).map_err(rs_err_to_py_err)?;
                req.method = scrapy_rs_core::request::Method::DELETE;
                req
            }
            Some("HEAD") => {
                let mut req = Request::get(url.as_str()).map_err(rs_err_to_py_err)?;
                req.method = scrapy_rs_core::request::Method::HEAD;
                req
            }
            Some("OPTIONS") => {
                let mut req = Request::get(url.as_str()).map_err(rs_err_to_py_err)?;
                req.method = scrapy_rs_core::request::Method::OPTIONS;
                req
            }
            Some("PATCH") => {
                if let Some(body_data) = body {
                    let mut req = Request::get(url.as_str()).map_err(rs_err_to_py_err)?;
                    req.method = scrapy_rs_core::request::Method::PATCH;
                    req.body = Some(body_data.to_vec());
                    req
                } else {
                    return Err(PyValueError::new_err("PATCH request requires a body"));
                }
            }
            Some(m) => {
                return Err(PyValueError::new_err(format!("Unsupported HTTP method: {}", m)));
            }
            None => Request::get(url.as_str()).map_err(rs_err_to_py_err)?,
        };
        
        // Add headers if provided
        if let Some(headers_dict) = headers {
            let headers_map = py_dict_to_hashmap(headers_dict)?;
            for (key, value) in headers_map {
                request.headers.insert(key, value);
            }
        }
        
        // Add cookies if provided
        if let Some(cookies_dict) = cookies {
            let cookies_map = py_dict_to_hashmap(cookies_dict)?;
            for (key, value) in cookies_map {
                request.cookies.insert(key, value);
            }
        }
        
        // Set dont_filter if provided
        if let Some(df) = dont_filter {
            request.dont_filter = df;
        }
        
        // Set timeout if provided
        if let Some(t) = timeout {
            request.timeout = Some(Duration::from_secs_f64(t));
        }
        
        // Set encoding if provided
        if let Some(enc) = encoding {
            request.encoding = Some(enc.to_string());
        }
        
        // Set flags if provided
        if let Some(f) = flags {
            request.flags = f;
        }
        
        // Set proxy if provided
        if let Some(p) = proxy {
            request.proxy = Some(p.to_string());
        }
        
        Ok(Self { inner: request })
    }
    
    /// Get the URL of the request
    #[getter]
    fn url(&self) -> String {
        self.inner.url.to_string()
    }
    
    /// Get the method of the request
    #[getter]
    fn method(&self) -> String {
        match self.inner.method {
            scrapy_rs_core::request::Method::GET => "GET".to_string(),
            scrapy_rs_core::request::Method::POST => "POST".to_string(),
            scrapy_rs_core::request::Method::PUT => "PUT".to_string(),
            scrapy_rs_core::request::Method::DELETE => "DELETE".to_string(),
            scrapy_rs_core::request::Method::HEAD => "HEAD".to_string(),
            scrapy_rs_core::request::Method::OPTIONS => "OPTIONS".to_string(),
            scrapy_rs_core::request::Method::PATCH => "PATCH".to_string(),
        }
    }
    
    /// Get the headers of the request
    #[getter]
    fn headers(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        
        for (key, value) in &self.inner.headers {
            dict.set_item(key, value)?;
        }
        
        Ok(dict.to_object(py))
    }
    
    /// Get the body of the request
    #[getter]
    fn body(&self) -> Option<Vec<u8>> {
        self.inner.body.clone()
    }
    
    /// Get the metadata of the request
    #[getter]
    fn meta(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        
        for (key, value) in &self.inner.meta {
            dict.set_item(key, json_to_py(py, value)?)?;
        }
        
        Ok(dict.to_object(py))
    }
    
    /// Set metadata for the request
    fn set_meta(&mut self, key: &str, value: &PyAny) -> PyResult<()> {
        let json_value = py_to_json_value(value)?;
        self.inner.meta.insert(key.to_string(), json_value);
        Ok(())
    }
    
    /// Set the callback for the request
    fn set_callback(&mut self, callback: &str) -> PyResult<()> {
        self.inner.callback = Some(callback.to_string());
        Ok(())
    }
    
    /// Set the errback for the request
    fn set_errback(&mut self, errback: &str) -> PyResult<()> {
        self.inner.errback = Some(errback.to_string());
        Ok(())
    }
    
    /// Set the priority for the request
    fn set_priority(&mut self, priority: i32) -> PyResult<()> {
        self.inner.priority = priority;
        Ok(())
    }
    
    /// Get the cookies of the request
    #[getter]
    fn cookies(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        
        for (key, value) in &self.inner.cookies {
            dict.set_item(key, value)?;
        }
        
        Ok(dict.to_object(py))
    }
    
    /// Set a cookie for the request
    fn set_cookie(&mut self, name: &str, value: &str) -> PyResult<()> {
        self.inner.cookies.insert(name.to_string(), value.to_string());
        Ok(())
    }
    
    /// Get the dont_filter flag
    #[getter]
    fn dont_filter(&self) -> bool {
        self.inner.dont_filter
    }
    
    /// Set the dont_filter flag
    fn set_dont_filter(&mut self, dont_filter: bool) -> PyResult<()> {
        self.inner.dont_filter = dont_filter;
        Ok(())
    }
    
    /// Get the timeout in seconds
    #[getter]
    fn timeout(&self) -> Option<f64> {
        self.inner.timeout.map(|d| d.as_secs_f64())
    }
    
    /// Set the timeout in seconds
    fn set_timeout(&mut self, timeout_secs: f64) -> PyResult<()> {
        self.inner.timeout = Some(Duration::from_secs_f64(timeout_secs));
        Ok(())
    }
    
    /// Get the encoding
    #[getter]
    fn encoding(&self) -> Option<String> {
        self.inner.encoding.clone()
    }
    
    /// Set the encoding
    fn set_encoding(&mut self, encoding: &str) -> PyResult<()> {
        self.inner.encoding = Some(encoding.to_string());
        Ok(())
    }
    
    /// Get the flags
    #[getter]
    fn flags(&self) -> Vec<String> {
        self.inner.flags.clone()
    }
    
    /// Add a flag to the request
    fn add_flag(&mut self, flag: &str) -> PyResult<()> {
        self.inner.flags.push(flag.to_string());
        Ok(())
    }
    
    /// Check if a flag is present
    fn has_flag(&self, flag: &str) -> bool {
        self.inner.has_flag(flag)
    }
    
    /// Get the proxy URL
    #[getter]
    fn proxy(&self) -> Option<String> {
        self.inner.proxy.clone()
    }
    
    /// Set the proxy URL
    fn set_proxy(&mut self, proxy: &str) -> PyResult<()> {
        self.inner.proxy = Some(proxy.to_string());
        Ok(())
    }
    
    /// Get the download latency in seconds
    #[getter]
    fn download_latency(&self) -> Option<f64> {
        self.inner.download_latency.map(|d| d.as_secs_f64())
    }
    
    fn __repr__(&self) -> String {
        format!("Request(url={}, method={})", self.inner.url, self.method())
    }
}

/// Python wrapper for Response
#[pyclass]
struct PyResponse {
    inner: Response,
}

impl PyResponse {
    fn new(response: Response) -> Self {
        Self { inner: response }
    }
}

#[pymethods]
impl PyResponse {
    #[new]
    #[pyo3(signature = (
        url,
        status=200,
        headers=None,
        body=b"",
        request=None,
        flags=None,
        certificate=None,
        ip_address=None,
        protocol=None
    ))]
    fn py_new(
        url: &str,
        status: u16,
        headers: Option<&PyDict>,
        body: Vec<u8>,
        request: Option<&PyRequest>,
        flags: Option<Vec<String>>,
        certificate: Option<Vec<u8>>,
        ip_address: Option<&str>,
        protocol: Option<&str>,
        py: Python,
    ) -> PyResult<Self> {
        // Parse URL
        let parsed_url = Url::parse(url).map_err(|e| {
            PyValueError::new_err(format!("Invalid URL: {}", e))
        })?;
        
        // Convert headers
        let mut headers_map = HashMap::new();
        if let Some(h) = headers {
            for (key, value) in h.iter() {
                let key = key.extract::<String>()?;
                let value = value.extract::<String>()?;
                headers_map.insert(key, value);
            }
        }
        
        // Get request or create a default one
        let req = match request {
            Some(r) => r.inner.clone(),
            None => Request::get(url).map_err(rs_err_to_py_err)?,
        };
        
        // Create the response
        let mut response = Response::new(req, status, headers_map, body);
        
        // Add flags if provided
        if let Some(f) = flags {
            response.flags = f;
        }
        
        // Set certificate if provided
        if let Some(cert) = certificate {
            response.certificate = Some(cert);
        }
        
        // Set IP address if provided
        if let Some(ip) = ip_address {
            response.ip_address = Some(ip.to_string());
        }
        
        // Set protocol if provided
        if let Some(proto) = protocol {
            response.protocol = Some(proto.to_string());
        }
        
        Ok(Self { inner: response })
    }
    
    /// Get the URL of the response
    #[getter]
    fn url(&self) -> String {
        self.inner.url.to_string()
    }
    
    /// Get the status code of the response
    #[getter]
    fn status(&self) -> u16 {
        self.inner.status
    }
    
    /// Get the headers of the response
    #[getter]
    fn headers(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        
        for (key, value) in &self.inner.headers {
            dict.set_item(key, value)?;
        }
        
        Ok(dict.to_object(py))
    }
    
    /// Get the body of the response as bytes
    #[getter]
    fn body(&self) -> Vec<u8> {
        self.inner.body.clone()
    }
    
    /// Get the body of the response as text
    fn text(&self) -> PyResult<String> {
        self.inner.text().map_err(rs_err_to_py_err)
    }
    
    /// Get the body of the response as JSON
    fn json(&self, py: Python) -> PyResult<PyObject> {
        let value: serde_json::Value = self.inner.json().map_err(rs_err_to_py_err)?;
        json_to_py(py, &value)
    }
    
    /// Get the request that generated this response
    #[getter]
    fn request(&self) -> PyRequest {
        PyRequest {
            inner: self.inner.request.clone(),
        }
    }
    
    /// Get the metadata of the response
    #[getter]
    fn meta(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        
        for (key, value) in &self.inner.meta {
            dict.set_item(key, json_to_py(py, value)?)?;
        }
        
        Ok(dict.to_object(py))
    }
    
    /// Get the flags of the response
    #[getter]
    fn flags(&self) -> Vec<String> {
        self.inner.flags.clone()
    }
    
    /// Add a flag to the response
    fn add_flag(&mut self, flag: &str) -> PyResult<()> {
        self.inner.flags.push(flag.to_string());
        Ok(())
    }
    
    /// Check if a flag is present
    fn has_flag(&self, flag: &str) -> bool {
        self.inner.has_flag(flag)
    }
    
    /// Get the certificate of the response
    #[getter]
    fn certificate(&self) -> Option<Vec<u8>> {
        self.inner.certificate.clone()
    }
    
    /// Get the IP address of the response
    #[getter]
    fn ip_address(&self) -> Option<String> {
        self.inner.ip_address.clone()
    }
    
    /// Get the protocol of the response
    #[getter]
    fn protocol(&self) -> Option<String> {
        self.inner.protocol.clone()
    }
    
    /// Check if the response was successful
    fn is_success(&self) -> bool {
        self.inner.is_success()
    }
    
    /// Check if the response is a redirect
    fn is_redirect(&self) -> bool {
        self.inner.is_redirect()
    }
    
    /// Join this response's URL with a relative URL
    fn urljoin(&self, url: &str) -> PyResult<String> {
        let joined_url = self.inner.urljoin(url).map_err(rs_err_to_py_err)?;
        Ok(joined_url.to_string())
    }
    
    /// Create a new request from a URL found in this response
    fn follow(&self, url: &str) -> PyResult<PyRequest> {
        let request = self.inner.follow(url).map_err(rs_err_to_py_err)?;
        Ok(PyRequest { inner: request })
    }
    
    /// Create multiple requests from URLs found in this response
    fn follow_all(&self, urls: Vec<&str>) -> PyResult<Vec<PyRequest>> {
        let requests = self.inner.follow_all(urls).map_err(rs_err_to_py_err)?;
        Ok(requests.into_iter().map(|req| PyRequest { inner: req }).collect())
    }
    
    /// Create a copy of this response
    fn copy(&self) -> Self {
        Self {
            inner: self.inner.copy(),
        }
    }
    
    /// Create a new response with modified attributes
    #[pyo3(signature = (
        url=None, 
        status=None, 
        headers=None, 
        body=None, 
        request=None, 
        meta=None, 
        flags=None, 
        certificate=None, 
        ip_address=None, 
        protocol=None
    ))]
    fn replace(
        &self,
        url: Option<&str>,
        status: Option<u16>,
        headers: Option<&PyDict>,
        body: Option<Vec<u8>>,
        request: Option<&PyRequest>,
        meta: Option<&PyDict>,
        flags: Option<Vec<String>>,
        certificate: Option<Vec<u8>>,
        ip_address: Option<&str>,
        protocol: Option<&str>,
        py: Python,
    ) -> PyResult<Self> {
        // Convert Python types to Rust types
        let url_opt = if let Some(u) = url {
            Some(Url::parse(u).map_err(|e| {
                PyValueError::new_err(format!("Invalid URL: {}", e))
            })?)
        } else {
            None
        };
        
        let headers_opt = if let Some(h) = headers {
            let mut map = HashMap::new();
            for (key, value) in h.iter() {
                let key = key.extract::<String>()?;
                let value = value.extract::<String>()?;
                map.insert(key, value);
            }
            Some(map)
        } else {
            None
        };
        
        let request_opt = request.map(|r| r.inner.clone());
        
        let meta_opt = if let Some(m) = meta {
            let mut map = HashMap::new();
            for (key, value) in m.iter() {
                let key = key.extract::<String>()?;
                let json_value = py_to_json_value(value)?;
                map.insert(key, json_value);
            }
            Some(map)
        } else {
            None
        };
        
        let certificate_opt = certificate.map(Some);
        
        let ip_address_opt = ip_address.map(|ip| Some(ip.to_string()));
        
        let protocol_opt = protocol.map(|p| Some(p.to_string()));
        
        // Create the new response
        let new_response = self.inner.replace(
            url_opt,
            status,
            headers_opt,
            body,
            request_opt,
            meta_opt,
            flags,
            certificate_opt,
            ip_address_opt,
            protocol_opt,
        );
        
        Ok(Self {
            inner: new_response,
        })
    }
    
    fn __repr__(&self) -> String {
        format!("Response(url={}, status={})", self.inner.url, self.inner.status)
    }
}

/// Python wrapper for Item
#[pyclass]
struct PyItem {
    inner: DynamicItem,
}

#[pymethods]
impl PyItem {
    #[new]
    fn new(item_type: &str) -> Self {
        Self {
            inner: DynamicItem::new(item_type),
        }
    }
    
    /// Get the type of the item
    #[getter]
    fn item_type(&self) -> String {
        self.inner.item_type_name.clone()
    }
    
    /// Get a field value
    fn get(&self, py: Python, key: &str) -> PyResult<PyObject> {
        match self.inner.get(key) {
            Some(value) => json_to_py(py, value),
            None => Ok(py.None()),
        }
    }
    
    /// Set a field value
    fn set(&mut self, key: &str, value: &PyAny) -> PyResult<()> {
        let json_value = py_to_json_value(value)?;
        self.inner.set(key, json_value);
        Ok(())
    }
    
    /// Check if a field exists
    fn has_field(&self, key: &str) -> bool {
        self.inner.has_field(key)
    }
    
    /// Get all fields as a dictionary
    #[getter]
    fn fields(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        
        for (key, value) in &self.inner.fields {
            dict.set_item(key, json_to_py(py, value)?)?;
        }
        
        Ok(dict.to_object(py))
    }
    
    fn __repr__(&self) -> String {
        format!("Item(type={})", self.inner.item_type_name)
    }
}

/// Python wrapper for Spider
#[pyclass]
struct PySpider {
    inner: Arc<dyn Spider>,
    runtime: Runtime,
}

#[pymethods]
impl PySpider {
    #[new]
    fn new(name: &str, start_urls: Vec<String>, allowed_domains: Option<Vec<String>>) -> PyResult<Self> {
        let mut spider = BasicSpider::new(name, start_urls);
        
        if let Some(domains) = allowed_domains {
            spider = spider.with_allowed_domains(domains);
        }
        
        let runtime = Runtime::new().map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to create Tokio runtime: {}", e))
        })?;
        
        Ok(Self {
            inner: Arc::new(spider),
            runtime,
        })
    }
    
    /// Get the name of the spider
    #[getter]
    fn name(&self) -> String {
        self.inner.name().to_string()
    }
    
    /// Get the allowed domains for this spider
    #[getter]
    fn allowed_domains(&self) -> Vec<String> {
        self.inner.allowed_domains()
    }
    
    /// Get the start URLs for this spider
    #[getter]
    fn start_urls(&self) -> Vec<String> {
        self.inner.start_urls()
    }
    
    /// Parse a response (this is a simple implementation that returns no items or requests)
    fn parse(&self, response: &PyResponse) -> PyResult<(Vec<PyItem>, Vec<PyRequest>)> {
        let parse_output = self.runtime.block_on(self.inner.parse(response.inner.clone()))
            .map_err(rs_err_to_py_err)?;
        
        let items = parse_output.items.into_iter()
            .map(|item| {
                // DynamicItem is now directly used, no need for downcasting
                PyItem {
                    inner: item,
                }
            })
            .collect();
        
        let requests = parse_output.requests.into_iter()
            .map(|req| PyRequest { inner: req })
            .collect();
        
        Ok((items, requests))
    }
}

/// Python wrapper for Engine
#[pyclass]
struct PyEngine {
    inner: Engine,
    runtime: Runtime,
}

#[pymethods]
impl PyEngine {
    #[new]
    #[pyo3(signature = (spider, config = None, downloader = None, scheduler = None))]
    fn new(
        spider: &PySpider,
        config: Option<&PyEngineConfig>,
        downloader: Option<&PyDownloader>,
        scheduler: Option<&PyScheduler>,
    ) -> PyResult<Self> {
        let runtime = Runtime::new().map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to create Tokio runtime: {}", e))
        })?;
        
        let engine = runtime.block_on(async {
            // Create basic engine
            let mut engine = Engine::new(spider.inner.clone())
                .map_err(rs_err_to_py_err)?;
            
            // Apply configuration if provided
            if let Some(config) = config {
                engine = engine.with_config(config.inner.clone());
            }
            
            // Create complete engine if downloader and scheduler are provided
            if let (Some(downloader), Some(scheduler)) = (downloader, scheduler) {
                // Create pipeline
                let pipeline = Arc::new(LogPipeline::info());
                
                // Create request middleware
                let request_middleware = Arc::new(DefaultHeadersMiddleware::common());
                
                // Create response middleware
                let response_middleware = Arc::new(ResponseLoggerMiddleware::info());
                
                // Create engine configuration
                let engine_config = match config {
                    Some(config) => config.inner.clone(),
                    None => EngineConfig::default(),
                };
                
                // Create complete engine
                engine = Engine::with_components(
                    spider.inner.clone(),
                    scheduler.inner.clone(),
                    downloader.inner.clone(),
                    pipeline,
                    request_middleware,
                    response_middleware,
                    engine_config,
                );
            }
            
            Ok::<_, PyErr>(engine)
        })?;
        
        Ok(Self {
            inner: engine,
            runtime,
        })
    }
    
    /// Run the engine
    fn run(&mut self) -> PyResult<PyEngineStats> {
        let stats = self.runtime.block_on(self.inner.run())
            .map_err(rs_err_to_py_err)?;
        
        Ok(PyEngineStats { inner: stats })
    }
    
    /// Get the current engine statistics
    fn stats(&self) -> PyResult<PyEngineStats> {
        let stats = self.runtime.block_on(self.inner.stats());
        Ok(PyEngineStats { inner: stats })
    }
    
    /// Check if the engine is running
    fn is_running(&self) -> PyResult<bool> {
        let running = self.runtime.block_on(self.inner.is_running());
        Ok(running)
    }
}

/// Python wrapper for EngineStats
#[pyclass]
struct PyEngineStats {
    inner: EngineStats,
}

#[pymethods]
impl PyEngineStats {
    /// Get the number of requests sent
    #[getter]
    fn request_count(&self) -> usize {
        self.inner.request_count
    }
    
    /// Get the number of responses received
    #[getter]
    fn response_count(&self) -> usize {
        self.inner.response_count
    }
    
    /// Get the number of items scraped
    #[getter]
    fn item_count(&self) -> usize {
        self.inner.item_count
    }
    
    /// Get the number of errors
    #[getter]
    fn error_count(&self) -> usize {
        self.inner.error_count
    }
    
    /// Get the duration of the crawl in seconds
    #[getter]
    fn duration_seconds(&self) -> Option<f64> {
        self.inner.duration().map(|d| d.as_secs_f64())
    }
    
    /// Get the requests per second
    #[getter]
    fn requests_per_second(&self) -> Option<f64> {
        self.inner.requests_per_second()
    }
    
    fn __repr__(&self) -> String {
        format!(
            "EngineStats(requests={}, responses={}, items={}, errors={})",
            self.inner.request_count,
            self.inner.response_count,
            self.inner.item_count,
            self.inner.error_count
        )
    }
}

/// Python wrapper for Settings
#[pyclass]
struct PySettings {
    inner: RsSettings,
}

#[pymethods]
impl PySettings {
    #[new]
    fn new() -> Self {
        Self {
            inner: RsSettings::new(),
        }
    }
    
    /// Load settings from a file
    #[staticmethod]
    fn from_file(path: &str) -> PyResult<Self> {
        let settings = RsSettings::from_file(path)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to load settings: {}", e)))?;
        
        Ok(Self {
            inner: settings,
        })
    }
    
    /// Load settings from a Python module
    #[staticmethod]
    fn from_module(module: &PyAny) -> PyResult<Self> {
        let mut settings = RsSettings::new();
        
        // Get all attribute names using Python
        let py = module.py();
        let locals = PyDict::new(py);
        locals.set_item("obj", module)?;
        let code = "[attr for attr in dir(obj) if attr.isupper() or attr == '_']";
        let attrs: Vec<String> = py.eval(code, None, Some(locals))?.extract()?;
        
        for attr_name in attrs {
            // Get attribute value
            let attr = module.getattr(attr_name.as_str())?;
            
            // Convert to JSON value
            let json_value = py_to_json_value(attr)?;
            
            // Set configuration item
            settings.raw.insert(attr_name, json_value);
        }
        
        Ok(Self {
            inner: settings,
        })
    }
    
    /// Get a setting
    fn get(&self, py: Python, key: &str) -> PyResult<PyObject> {
        match self.inner.raw.get(key) {
            Some(value) => json_to_py(py, value),
            None => Err(PyValueError::new_err(format!("Setting not found: {}", key))),
        }
    }
    
    /// Get a setting with a default value
    fn get_or(&self, py: Python, key: &str, default: &PyAny) -> PyResult<PyObject> {
        match self.inner.raw.get(key) {
            Some(value) => json_to_py(py, value),
            None => Ok(default.to_object(py)),
        }
    }
    
    /// Set a setting
    fn set(&mut self, key: &str, value: &PyAny) -> PyResult<()> {
        let json_value = py_to_json_value(value)?;
        self.inner.raw.insert(key.to_string(), json_value);
        Ok(())
    }
    
    /// Check if a setting exists
    fn contains(&self, key: &str) -> bool {
        self.inner.contains(key)
    }
    
    /// Remove a setting
    fn remove(&mut self, key: &str) -> bool {
        self.inner.remove(key).is_some()
    }
    
    /// Get all settings
    fn all(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        
        for (key, value) in self.inner.all() {
            let py_value = json_to_py(py, value)?;
            dict.set_item(key, py_value)?;
        }
        
        Ok(dict.to_object(py))
    }
    
    /// Save settings to a file
    fn save(&self, path: &str) -> PyResult<()> {
        self.inner.save(path)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to save settings: {}", e)))
    }
    
    /// String representation
    fn __repr__(&self) -> String {
        format!("PySettings with {} settings", self.inner.all().len())
    }
    
    /// Create a spider from settings
    fn create_spider(&self) -> PyResult<PySpider> {
        let runtime = Runtime::new().map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to create Tokio runtime: {}", e))
        })?;
        
        let spider = create_spider_from_settings(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create spider: {}", e)))?;
        
        Ok(PySpider {
            inner: spider,
            runtime,
        })
    }
    
    /// Create a downloader from settings
    fn create_downloader(&self) -> PyResult<PyDownloader> {
        let downloader = create_downloader_from_settings(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create downloader: {}", e)))?;
        
        Ok(PyDownloader { inner: downloader })
    }
    
    /// Create a scheduler from settings
    fn create_scheduler(&self) -> PyResult<PyScheduler> {
        let scheduler = create_scheduler_from_settings(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create scheduler: {}", e)))?;
        
        Ok(PyScheduler { inner: scheduler })
    }
    
    /// Create an engine from settings
    fn create_engine(&self, spider: &PySpider) -> PyResult<PyEngine> {
        let runtime = Runtime::new().map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to create Tokio runtime: {}", e))
        })?;
        
        // Create configuration
        let config = engine_config_from_settings(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create engine config: {}", e)))?;
        
        // Create downloader
        let downloader = create_downloader_from_settings(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create downloader: {}", e)))?;
        
        // Create scheduler
        let scheduler = create_scheduler_from_settings(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create scheduler: {}", e)))?;
        
        // Create pipeline
        let pipeline = Arc::new(LogPipeline::info());
        
        // Create request middleware
        let request_middleware = Arc::new(DefaultHeadersMiddleware::common());
        
        // Create response middleware
        let response_middleware = Arc::new(ResponseLoggerMiddleware::info());
        
        // Create engine
        let engine = Engine::with_components(
            spider.inner.clone(),
            scheduler,
            downloader,
            pipeline,
            request_middleware,
            response_middleware,
            config,
        );
        
        Ok(PyEngine {
            inner: engine,
            runtime,
        })
    }
}

/// Python wrapper for DownloaderConfig
#[pyclass]
struct PyDownloaderConfig {
    inner: DownloaderConfig,
}

#[pymethods]
impl PyDownloaderConfig {
    #[new]
    fn new() -> Self {
        Self {
            inner: DownloaderConfig::default(),
        }
    }
    
    /// Get the concurrent requests setting
    #[getter]
    fn get_concurrent_requests(&self) -> usize {
        self.inner.concurrent_requests
    }
    
    /// Set the concurrent requests setting
    #[setter]
    fn set_concurrent_requests(&mut self, value: usize) {
        self.inner.concurrent_requests = value;
    }
    
    /// Get the user agent setting
    #[getter]
    fn get_user_agent(&self) -> String {
        self.inner.user_agent.clone()
    }
    
    /// Set the user agent setting
    #[setter]
    fn set_user_agent(&mut self, value: String) {
        self.inner.user_agent = value;
    }
    
    /// Get the timeout setting
    #[getter]
    fn get_timeout(&self) -> u64 {
        self.inner.timeout
    }
    
    /// Set the timeout setting
    #[setter]
    fn set_timeout(&mut self, value: u64) {
        self.inner.timeout = value;
    }
    
    /// String representation
    fn __repr__(&self) -> String {
        format!("PyDownloaderConfig(concurrent_requests={}, timeout={})",
            self.inner.concurrent_requests, self.inner.timeout)
    }
}

/// Python wrapper for Downloader
#[pyclass]
struct PyDownloader {
    inner: Arc<HttpDownloader>,
}

#[pymethods]
impl PyDownloader {
    #[new]
    fn new(config: Option<&PyDownloaderConfig>) -> PyResult<Self> {
        let config = match config {
            Some(config) => config.inner.clone(),
            None => DownloaderConfig::default(),
        };
        
        let downloader = HttpDownloader::new(config)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create downloader: {}", e)))?;
        
        Ok(Self {
            inner: Arc::new(downloader),
        })
    }
    
    /// String representation
    fn __repr__(&self) -> String {
        format!("PyDownloader()")
    }
}

/// Python wrapper for Scheduler
#[pyclass]
struct PyScheduler {
    inner: Arc<MemoryScheduler>,
}

#[pymethods]
impl PyScheduler {
    #[new]
    fn new() -> Self {
        Self {
            inner: Arc::new(MemoryScheduler::new()),
        }
    }
    
    /// String representation
    fn __repr__(&self) -> String {
        format!("PyScheduler()")
    }
}

/// Python wrapper for EngineConfig
#[pyclass]
struct PyEngineConfig {
    inner: EngineConfig,
}

#[pymethods]
impl PyEngineConfig {
    #[new]
    fn new() -> Self {
        Self {
            inner: EngineConfig::default(),
        }
    }
    
    /// Get the concurrent requests setting
    #[getter]
    fn get_concurrent_requests(&self) -> usize {
        self.inner.concurrent_requests
    }
    
    /// Set the concurrent requests setting
    #[setter]
    fn set_concurrent_requests(&mut self, value: usize) {
        self.inner.concurrent_requests = value;
    }
    
    /// Get the concurrent items setting
    #[getter]
    fn get_concurrent_items(&self) -> usize {
        self.inner.concurrent_items
    }
    
    /// Set the concurrent items setting
    #[setter]
    fn set_concurrent_items(&mut self, value: usize) {
        self.inner.concurrent_items = value;
    }
    
    /// Get the download delay setting
    #[getter]
    fn get_download_delay_ms(&self) -> u64 {
        self.inner.download_delay_ms
    }
    
    /// Set the download delay setting
    #[setter]
    fn set_download_delay_ms(&mut self, value: u64) {
        self.inner.download_delay_ms = value;
    }
    
    /// Get the user agent setting
    #[getter]
    fn get_user_agent(&self) -> String {
        self.inner.user_agent.clone()
    }
    
    /// Set the user agent setting
    #[setter]
    fn set_user_agent(&mut self, value: String) {
        self.inner.user_agent = value;
    }
    
    /// Get the respect robots.txt setting
    #[getter]
    fn get_respect_robots_txt(&self) -> bool {
        self.inner.respect_robots_txt
    }
    
    /// Set the respect robots.txt setting
    #[setter]
    fn set_respect_robots_txt(&mut self, value: bool) {
        self.inner.respect_robots_txt = value;
    }
    
    /// Get the follow redirects setting
    #[getter]
    fn get_follow_redirects(&self) -> bool {
        self.inner.follow_redirects
    }
    
    /// Set the follow redirects setting
    #[setter]
    fn set_follow_redirects(&mut self, value: bool) {
        self.inner.follow_redirects = value;
    }
    
    /// String representation
    fn __repr__(&self) -> String {
        format!("PyEngineConfig(concurrent_requests={}, download_delay_ms={})",
            self.inner.concurrent_requests, self.inner.download_delay_ms)
    }
}

// Helper function to handle Box<Error> to PyErr conversion
fn boxed_rs_err_to_py_err(err: Box<scrapy_rs_core::error::Error>) -> PyErr {
    rs_err_to_py_err(*err)
} 