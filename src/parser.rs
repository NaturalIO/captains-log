use regex::Regex;
use std::fs::File;
use std::io::{BufRead, BufReader, Lines, Result as IoResult};

/// A simple LogParser with custom regex pattern
pub struct LogParser {
    reader: BufReader<File>,
    re: Regex,
}

impl LogParser {
    pub fn new(file_path: &str, re_pattern: &str, buf_size: usize) -> IoResult<Self> {
        let f = File::open(file_path)?;
        let reader = BufReader::with_capacity(buf_size, f);
        let re = Regex::new(re_pattern).expect("regex pattern valid");
        Ok(Self { reader, re })
    }

    pub fn lines(self) -> LogParserLineIter {
        LogParserLineIter { lines: self.reader.lines(), re: self.re }
    }
}

pub struct LogParserLineIter {
    re: Regex,
    lines: Lines<BufReader<File>>,
}

impl Iterator for LogParserLineIter {
    type Item = IoResult<Vec<String>>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let line;
            match self.lines.next() {
                None => return None,
                Some(Err(e)) => return Some(Err(e)),
                Some(Ok(_line)) => {
                    line = _line;
                }
            }
            if let Some(caps) = self.re.captures(&line) {
                let mut line_result = Vec::with_capacity(caps.len());
                for m in caps.iter() {
                    if let Some(mat) = m {
                        line_result.push(mat.as_str().to_string());
                    } else {
                        line_result.push("".to_string());
                    }
                }
                return Some(Ok(line_result));
            }
            // Ignore unrecognized format
        }
    }
}
