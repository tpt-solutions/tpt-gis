//! Minimal, dependency-free CSV writer for `spatial-join` CSV output.
//!
//! Implements RFC-4180-style quoting: a field is wrapped in double quotes
//! (with embedded quotes doubled) when it contains a comma, quote, CR, or LF.
//! This replaces the external `csv` crate for our single, simple use case.

pub struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    pub fn from_writer(buf: Vec<u8>) -> Self {
        Writer { buf }
    }

    pub fn write_record<'a>(
        &mut self,
        fields: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), String> {
        let mut first = true;
        for field in fields {
            if !first {
                self.buf.push(b',');
            }
            first = false;
            self.write_field(field)?;
        }
        self.buf.push(b'\n');
        Ok(())
    }

    fn write_field(&mut self, field: &str) -> Result<(), String> {
        let needs_quote =
            field.as_bytes().iter().any(|&b| b == b',' || b == b'"' || b == b'\n' || b == b'\r');
        if needs_quote {
            self.buf.push(b'"');
            for &b in field.as_bytes() {
                if b == b'"' {
                    self.buf.push(b'"');
                }
                self.buf.push(b);
            }
            self.buf.push(b'"');
        } else {
            self.buf.extend_from_slice(field.as_bytes());
        }
        Ok(())
    }

    pub fn into_inner(self) -> Result<Vec<u8>, String> {
        Ok(self.buf)
    }
}

#[cfg(test)]
mod tests {
    use super::Writer;

    #[test]
    fn plain_fields() {
        let mut w = Writer::from_writer(vec![]);
        w.write_record(["1.0", "2.0", "x"]).unwrap();
        w.write_record(["3.0", "4.0", "y"]).unwrap();
        assert_eq!(String::from_utf8(w.into_inner().unwrap()).unwrap(), "1.0,2.0,x\n3.0,4.0,y\n");
    }

    #[test]
    fn quotes_special_chars() {
        let mut w = Writer::from_writer(vec![]);
        w.write_record(["a,b", "he said \"hi\"", "line\nbreak"]).unwrap();
        assert_eq!(
            String::from_utf8(w.into_inner().unwrap()).unwrap(),
            "\"a,b\",\"he said \"\"hi\"\"\",\"line\nbreak\"\n"
        );
    }
}
