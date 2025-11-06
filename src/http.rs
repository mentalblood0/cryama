use regex::Regex;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;

pub struct Client {
    pub response_status_line_regex: Regex,
    pub response_header_line_regex: Regex,
}

#[derive(Debug)]
pub struct HttpResponseStatusLine {
    pub version: String,
    pub status_code: u16,
    pub status_message: String,
}

#[derive(Debug)]
pub struct HttpResponse {
    pub status_line: HttpResponseStatusLine,
    pub headers: HashMap<String, String>,
    pub body: String,
}

impl Client {
    fn read_response_status_line<R: Read>(
        &self,
        reader: &mut BufReader<R>,
    ) -> Result<HttpResponseStatusLine, String> {
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|error| format!("Can not read line of response: {}", error))?;
        line = line.trim_end().to_string();

        match self.response_status_line_regex.captures(line.as_str()) {
            Some(captures) => Ok(HttpResponseStatusLine {
                version: match captures.get(1) {
                    Some(version) => version.as_str().to_string(),
                    None => {
                        return Err(
                            "Can not get version from parsed status line of response".to_string()
                        );
                    }
                },
                status_code: match captures.get(2) {
                    Some(status_code) => match status_code.as_str().to_string().parse::<u16>() {
                        Ok(status_code) => status_code,
                        Err(error) => {
                            return Err(format!(
                                "Can not parse status code from response parsed status line {line} to number: {error}"
                            ));
                        }
                    },
                    None => {
                        return Err(format!(
                            "Can not get status code from response parsed status line {line}"
                        ));
                    }
                },
                status_message: match captures.get(3) {
                    Some(status_message) => status_message.as_str().to_string(),
                    None => {
                        return Err(format!(
                            "Can not get status message from response parsed status line {line}"
                        ));
                    }
                },
            }),
            None => Err(format!("Can not parse response status line {line}")),
        }
    }
    fn read_response_headers<R: Read>(
        &self,
        reader: &mut BufReader<R>,
    ) -> Result<HashMap<String, String>, String> {
        let mut result: HashMap<String, String> = HashMap::new();

        loop {
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .map_err(|error| format!("Can not read line of response: {}", error))?;
            line = line.trim_end().to_string();
            dbg!(&line);

            if let Some(captures) = self.response_header_line_regex.captures(line.as_str()) {
                let key = match captures.get(1) {
                    Some(version) => version,
                    None => {
                        return Err(format!(
                            "Can not get key from response parsed header line {line}"
                        ));
                    }
                };
                let value = match captures.get(2) {
                    Some(version) => version,
                    None => {
                        return Err(format!(
                            "Can not get value from response parsed header line {line}"
                        ));
                    }
                };
                result.insert(key.as_str().to_string(), value.as_str().to_string());
            } else {
                break;
            }
        }
        Ok(result)
    }
    pub fn send_request(
        &self,
        method: &str,
        host: &str,
        port: u16,
        path: &str,
        request_body: &str,
    ) -> Result<HttpResponse, String> {
        let mut stream =
            TcpStream::connect(format!("{}:{}", host, port).as_str()).map_err(|error| {
                format!("Can not connect to host {} port {}: {}", host, port, error)
            })?;

        let request = [
            format!("{} {} HTTP/1.1", method, path).as_str(),
            format!("Host: {}", host).as_str(),
            "Content-Type: application/json",
            format!("Content-Length: {}", request_body.len()).as_str(),
            "",
            request_body,
        ]
        .join("\r\n");

        stream
            .write_all(request.as_bytes())
            .map_err(|error| format!("Can not send request to: {error}"))?;
        stream
            .flush()
            .map_err(|error| format!("Can not flush request to: {error}"))?;

        let mut reader = BufReader::new(stream);
        let status_line = self.read_response_status_line(&mut reader)?;
        let headers = self.read_response_headers(&mut reader)?;

        if let Some(content_length_string) = headers.get("Content-Length") {
            let content_length = content_length_string.parse::<usize>().map_err(|error| {
                format!("Can not parse Content-Length value {content_length_string}: {error}")
            })?;
            let mut response_body = vec![0u8; content_length];
            reader.read_exact(&mut response_body).map_err(|error| {
                format!(
                    "Can not read response body of stated by Content-Length size {content_length}: {error}"
                )
            })?;
            let body = String::from_utf8_lossy(&response_body).into_owned();
            Ok(HttpResponse {
                status_line: status_line,
                headers: headers,
                body: body,
            })
        } else {
            Err("Can not read response content because Content-Length line not found".to_string())
        }
    }
}
