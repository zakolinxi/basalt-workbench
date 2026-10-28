use futures_util::StreamExt;
use reqwest::Response;
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};

pub struct LineDecoder {
    buffer: Vec<u8>,
}

impl LineDecoder {
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    pub fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buffer.extend_from_slice(chunk);
        let mut lines = Vec::new();
        loop {
            let Some(pos) = self.buffer.iter().position(|b| *b == b'\n') else {
                break;
            };
            let mut line: Vec<u8> = self.buffer.drain(..=pos).collect();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            lines.push(String::from_utf8_lossy(&line).to_string());
        }
        lines
    }

    pub fn finish(&mut self) -> Vec<String> {
        if self.buffer.is_empty() {
            return Vec::new();
        }
        let line = String::from_utf8_lossy(&self.buffer).to_string();
        self.buffer.clear();
        vec![line]
    }
}

pub fn sse_data(line: &str) -> Option<&str> {
    line.strip_prefix("data:").map(|d| d.trim_start())
}

pub async fn for_each_line<F>(
    response: Response,
    cancel: &CancellationToken,
    mut on_line: F,
) -> AppResult<()>
where
    F: FnMut(&str) -> AppResult<bool>,
{
    let mut stream = response.bytes_stream();
    let mut decoder = LineDecoder::new();
    loop {
        let next = tokio::select! {
            _ = cancel.cancelled() => return Err(AppError::Cancelled),
            next = stream.next() => next,
        };
        match next {
            Some(Ok(chunk)) => {
                for line in decoder.push(&chunk) {
                    if on_line(&line)? {
                        return Ok(());
                    }
                }
            }
            Some(Err(err)) => return Err(AppError::Http(err)),
            None => break,
        }
    }
    for line in decoder.finish() {
        if on_line(&line)? {
            return Ok(());
        }
    }
    Ok(())
}

pub fn parse_data_json(line: &str) -> Option<serde_json::Value> {
    let data = sse_data(line)?;
    if data.is_empty() || data == "[DONE]" {
        return None;
    }
    serde_json::from_str(data).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_lines_split_across_chunks() {
        let mut decoder = LineDecoder::new();
        assert!(decoder.push(b"data: {\"a\":").is_empty());
        let lines = decoder.push(b"1}\r\n\ndata: [DONE]\n");
        assert_eq!(lines.len(), 3);
        assert_eq!(sse_data(&lines[2]), Some("[DONE]"));
    }

    #[test]
    fn parses_sse_json() {
        let value = parse_data_json("data: {\"type\":\"x\"}").unwrap();
        assert_eq!(value["type"], "x");
    }
}
