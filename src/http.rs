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
    pub body: Vec<u8>,
}

macro_rules! read_line {
    ($reader:ident) => {{
        let mut line = String::new();
        $reader
            .read_line(&mut line)
            .map_err(|error| format!("Can not read line of response: {error}"))?;
        let line_trimmed = line.trim_end().to_string();
        line_trimmed
    }};
}

impl Client {
    fn read_response_status_line<R: Read>(
        &self,
        reader: &mut BufReader<R>,
    ) -> Result<HttpResponseStatusLine, String> {
        let line = read_line!(reader);
        let captures = self
            .response_status_line_regex
            .captures(line.as_str())
            .ok_or(format!("Can not parse response status line {line}"))?;
        Ok(HttpResponseStatusLine {
                version: captures
                    .get(1)
                    .ok_or(format!(
                        "Can not get version from response parsed status line {line}"
                    ))?
                    .as_str()
                    .to_string(),
                status_code: captures.get(2).ok_or(format!(
                                "Can not get status code from response parsed status line {line}"
                            ))?.as_str().to_string().parse::<u16>().map_err(|error| format!(
                                "Can not parse status code from response parsed status line {line} to number: {error}"
                            ))? ,
                status_message: captures.get(3).ok_or(format!(
                            "Can not get status message from response parsed status line {line}"
                        ))?.as_str().to_string()
            })
    }
    fn read_response_headers<R: Read>(
        &self,
        reader: &mut BufReader<R>,
    ) -> Result<HashMap<String, String>, String> {
        let mut result: HashMap<String, String> = HashMap::new();

        loop {
            let line = read_line!(reader);
            if let Some(captures) = self.response_header_line_regex.captures(line.as_str()) {
                result.insert(
                    captures
                        .get(1)
                        .ok_or(format!(
                            "Can not get key from response parsed header line {line}"
                        ))?
                        .as_str()
                        .to_string(),
                    captures
                        .get(2)
                        .ok_or(format!(
                            "Can not get value from response parsed header line {line}"
                        ))?
                        .as_str()
                        .to_string(),
                );
            } else {
                break;
            }
        }
        Ok(result)
    }
    fn read_response_fixed_body<R: Read>(
        &self,
        reader: &mut BufReader<R>,
        content_length: usize,
    ) -> Result<Vec<u8>, String> {
        let mut result = vec![0u8; content_length];
        reader.read_exact(&mut result).map_err(|error| format!("Can not read response body of stated by Content-Length size {content_length}: {error}"))?;
        Ok(result)
    }
    fn read_response_chunked_body<R: Read>(
        &self,
        reader: &mut BufReader<R>,
    ) -> Result<Vec<u8>, String> {
        let mut result: Vec<u8> = Vec::new();
        loop {
            let size_line = read_line!(reader);
            let chunk_size = usize::from_str_radix(&size_line, 16).map_err(|error| {
                format!("Can not parse response chunk size from line {size_line}: {error}")
            })?;
            if chunk_size == 0 {
                break;
            }
            {
                let mut chunk = vec![0u8; chunk_size];
                reader.read_exact(&mut chunk).map_err(|error| {
                    format!("Can not read response chunk of stated size {chunk_size}: {error}")
                })?;
                result.extend_from_slice(&chunk);
            }
            let mut line_break = vec![0u8; 2];
            reader.read_exact(&mut line_break).map_err(|error| {
                format!("Can not read line break after response chunk: {error}")
            })?;
        }
        Ok(result)
    }
    pub fn send_request(
        &self,
        method: &str,
        host: &str,
        port: u16,
        path: &str,
        content_type: &str,
        request_body: &str,
    ) -> Result<HttpResponse, String> {
        let mut stream = TcpStream::connect(format!("{host}:{port}").as_str())
            .map_err(|error| format!("Can not connect to host {host} port {port}: {error}"))?;

        let request = format!(
            "{method} {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n{request_body}",
            request_body.len()
        );

        stream
            .write_all(request.as_bytes())
            .map_err(|error| format!("Can not send request: {error}"))?;
        stream
            .flush()
            .map_err(|error| format!("Can not flush request: {error}"))?;

        let mut reader = BufReader::new(stream);
        let status_line = self.read_response_status_line(&mut reader)?;
        let headers = self.read_response_headers(&mut reader)?;
        let body = match headers.get("Content-Length") {
            Some(content_length_string) => {
                let content_length = content_length_string.parse::<usize>().map_err(|error| {
                    format!("Can not parse Content-Length value {content_length_string}: {error}")
                })?;
                self.read_response_fixed_body(&mut reader, content_length)?
            }
            None => {
                let transfer_encoding =  headers.get("Transfer-Encoding").ok_or("Can not read response content because Content-Length line not found and Transfer-Encoding is also not stated")?.as_str();
                match transfer_encoding {
                    "chunked" => self.read_response_chunked_body(&mut reader)?,
                    _ => {
                        return Err(format!(
                            "Can not read response content because Content-Length line not found and transfer encoding {transfer_encoding} is not supported"
                        ));
                    }
                }
            }
        };
        Ok(HttpResponse {
            status_line: status_line,
            headers: headers,
            body: body,
        })
    }
}
