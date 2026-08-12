//! W2-1 — 실 ARC-AGI-1 데이터 로더.
//!
//! ARC 과제 JSON은 극도로 제한된 형태다:
//! `{"train":[{"input":[[0..9]],"output":[[0..9]]},...],"test":[...]}`
//! 외부 의존성 0 원칙을 지키기 위해 이 부분집합만 읽는 손수 파서를 쓴다.

use crate::grid::Grid;

pub struct ArcPair {
    pub input: Grid,
    pub output: Grid,
}

pub struct ArcTask {
    pub name: String,
    pub train: Vec<ArcPair>,
    pub test: Vec<ArcPair>,
}

struct P<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> P<'a> {
    fn ws(&mut self) {
        while self.i < self.b.len() && (self.b[self.i] as char).is_whitespace() {
            self.i += 1;
        }
    }
    fn eat(&mut self, c: u8) -> Result<(), String> {
        self.ws();
        if self.i < self.b.len() && self.b[self.i] == c {
            self.i += 1;
            Ok(())
        } else {
            Err(format!("기대 {} @ {}", c as char, self.i))
        }
    }
    fn peek(&mut self) -> u8 {
        self.ws();
        if self.i < self.b.len() {
            self.b[self.i]
        } else {
            0
        }
    }
    fn string(&mut self) -> Result<String, String> {
        self.eat(b'"')?;
        let s = self.i;
        while self.i < self.b.len() && self.b[self.i] != b'"' {
            self.i += 1;
        }
        let out = String::from_utf8_lossy(&self.b[s..self.i]).into_owned();
        self.eat(b'"')?;
        Ok(out)
    }
    fn int(&mut self) -> Result<u8, String> {
        self.ws();
        let s = self.i;
        while self.i < self.b.len() && self.b[self.i].is_ascii_digit() {
            self.i += 1;
        }
        if s == self.i {
            return Err(format!("숫자 아님 @ {}", self.i));
        }
        std::str::from_utf8(&self.b[s..self.i])
            .ok()
            .and_then(|t| t.parse::<u8>().ok())
            .ok_or_else(|| "숫자 파싱".to_string())
    }
    fn grid(&mut self) -> Result<Grid, String> {
        self.eat(b'[')?;
        let mut rows: Vec<Vec<u8>> = Vec::new();
        loop {
            if self.peek() == b']' {
                self.i += 1;
                break;
            }
            self.eat(b'[')?;
            let mut row = Vec::new();
            loop {
                if self.peek() == b']' {
                    self.i += 1;
                    break;
                }
                row.push(self.int()?);
                if self.peek() == b',' {
                    self.i += 1;
                }
            }
            rows.push(row);
            if self.peek() == b',' {
                self.i += 1;
            }
        }
        let h = rows.len();
        let w = rows.first().map(|r| r.len()).unwrap_or(0);
        if h == 0 || w == 0 || rows.iter().any(|r| r.len() != w) {
            return Err("불규칙 격자".to_string());
        }
        let mut g = Grid::new(w, h);
        for (y, row) in rows.iter().enumerate() {
            for (x, &c) in row.iter().enumerate() {
                g.set(x, y, c);
            }
        }
        Ok(g)
    }
    fn pair(&mut self) -> Result<ArcPair, String> {
        self.eat(b'{')?;
        let mut input = None;
        let mut output = None;
        loop {
            if self.peek() == b'}' {
                self.i += 1;
                break;
            }
            let key = self.string()?;
            self.eat(b':')?;
            let g = self.grid()?;
            match key.as_str() {
                "input" => input = Some(g),
                "output" => output = Some(g),
                _ => return Err(format!("모르는 키 {key}")),
            }
            if self.peek() == b',' {
                self.i += 1;
            }
        }
        Ok(ArcPair {
            input: input.ok_or("input 없음")?,
            output: output.ok_or("output 없음")?,
        })
    }
    fn pairs(&mut self) -> Result<Vec<ArcPair>, String> {
        self.eat(b'[')?;
        let mut out = Vec::new();
        loop {
            if self.peek() == b']' {
                self.i += 1;
                break;
            }
            out.push(self.pair()?);
            if self.peek() == b',' {
                self.i += 1;
            }
        }
        Ok(out)
    }
}

pub fn parse_task(name: &str, json: &[u8]) -> Result<ArcTask, String> {
    let mut p = P { b: json, i: 0 };
    p.eat(b'{')?;
    let mut train = Vec::new();
    let mut test = Vec::new();
    loop {
        if p.peek() == b'}' {
            break;
        }
        let key = p.string()?;
        p.eat(b':')?;
        // 부가 키(예: "name": 문자열)는 값 형태를 보고 스킵
        if p.peek() == b'"' {
            let _ = p.string()?;
            if p.peek() == b',' {
                p.i += 1;
            }
            continue;
        }
        let v = p.pairs()?;
        match key.as_str() {
            "train" => train = v,
            "test" => test = v,
            _ => return Err(format!("모르는 키 {key}")),
        }
        if p.peek() == b',' {
            p.i += 1;
        }
    }
    Ok(ArcTask { name: name.to_string(), train, test })
}

pub fn load_dir(dir: &std::path::Path) -> Vec<ArcTask> {
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    names.sort();
    let mut out = Vec::new();
    for path in names {
        if let Ok(bytes) = std::fs::read(&path) {
            let name = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
            match parse_task(&name, &bytes) {
                Ok(t) => out.push(t),
                Err(e) => eprintln!("파싱 실패 {name}: {e}"),
            }
        }
    }
    out
}
