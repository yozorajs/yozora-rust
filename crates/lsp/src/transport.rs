use std::io::{self, BufRead, Read, Write};
use std::thread;

use serde_json::Value;

const MAX_HEADER_BYTES: usize = 8 * 1024;
const MAX_MESSAGE_BYTES: usize = 16 * 1024 * 1024;

/// Keep blocking reads off the server loop so idle diagnostics can run. One
/// queued frame provides backpressure instead of accumulating unbounded input.
/// This reader lives for the binary's process lifetime: joining it on exit could
/// block forever waiting for the client to close stdin.
pub fn read_messages(
    mut reader: impl BufRead + Send + 'static,
    mut publish: impl FnMut(io::Result<Option<Vec<u8>>>) -> bool + Send + 'static,
) -> io::Result<()> {
    thread::Builder::new()
        .name("yozora-lsp-input".into())
        .spawn(move || loop {
            let message = read_message(&mut reader);
            let terminal = !matches!(message, Ok(Some(_)));
            if !publish(message) || terminal {
                break;
            }
        })?;
    Ok(())
}

pub fn read_message(reader: &mut impl BufRead) -> io::Result<Option<Vec<u8>>> {
    let mut length = None;
    let mut header_bytes = 0;
    loop {
        let mut line = Vec::new();
        let read = reader
            .by_ref()
            .take((MAX_HEADER_BYTES - header_bytes + 1) as u64)
            .read_until(b'\n', &mut line)?;
        if read == 0 {
            return if header_bytes == 0 {
                Ok(None)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "incomplete LSP header",
                ))
            };
        }
        header_bytes += read;
        if header_bytes > MAX_HEADER_BYTES {
            return Err(invalid_data("LSP header exceeds 8 KiB"));
        }
        if line == b"\r\n" {
            break;
        }
        if !line.is_ascii() || !line.ends_with(b"\r\n") {
            return Err(invalid_data("LSP headers require ASCII and CRLF"));
        }
        let line = std::str::from_utf8(&line).map_err(invalid_data)?;
        let (name, value) = line
            .trim_end()
            .split_once(':')
            .ok_or_else(|| invalid_data("invalid LSP header"))?;
        if name.eq_ignore_ascii_case("Content-Length") {
            if length.is_some() {
                return Err(invalid_data("duplicate Content-Length"));
            }
            let value = value.trim();
            if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(invalid_data("invalid Content-Length"));
            }
            let value: usize = value.parse().map_err(invalid_data)?;
            if value > MAX_MESSAGE_BYTES {
                return Err(invalid_data("LSP message exceeds 16 MiB"));
            }
            length = Some(value);
        }
    }

    let length = length.ok_or_else(|| invalid_data("missing Content-Length"))?;
    let mut message = vec![0; length];
    reader.read_exact(&mut message)?;
    Ok(Some(message))
}

pub fn write_message(writer: &mut impl Write, message: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(message)?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()
}

fn invalid_data(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Cursor};

    use serde_json::json;

    use super::*;

    #[test]
    fn reads_back_to_back_unicode_messages_through_small_buffers() {
        let message = json!({"method": "example", "params": {"text": "中文😀"}});
        let mut bytes = Vec::new();
        write_message(&mut bytes, &message).unwrap();
        write_message(&mut bytes, &message).unwrap();
        let mut input = BufReader::with_capacity(3, Cursor::new(bytes));
        for _ in 0..2 {
            let body = read_message(&mut input).unwrap().unwrap();
            assert_eq!(serde_json::from_slice::<Value>(&body).unwrap(), message);
        }
        assert!(read_message(&mut input).unwrap().is_none());
    }

    #[test]
    fn accepts_extra_headers_and_case_insensitive_content_length() {
        let mut input = &b"Content-Type: application/vscode-jsonrpc; charset=utf-8\r\ncontent-length: 2\r\n\r\n{}"[..];
        assert_eq!(read_message(&mut input).unwrap(), Some(b"{}".to_vec()));
    }

    #[test]
    fn rejects_invalid_or_truncated_frames() {
        for input in [
            "\r\n{}",
            "Content-Length: 2\r\nContent-Length: 2\r\n\r\n{}",
            "Content-Length: -1\r\n\r\n",
            "Content-Length: +1\r\n\r\n",
            "Content-Length: 16777217\r\n\r\n",
            "Content-Length: 2\r\n",
            "Content-Length: 2\r\n\r\n{",
            "Content-Length: 2\n\n{}",
        ] {
            assert!(read_message(&mut input.as_bytes()).is_err(), "{input:?}");
        }
        let oversized = format!("X: {}\r\n\r\n", "x".repeat(MAX_HEADER_BYTES));
        assert!(read_message(&mut oversized.as_bytes()).is_err());
    }
}
