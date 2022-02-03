use debugserver_types::{Capabilities, InitializeRequest, InitializeResponse};
use fd_lock::RwLock;
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
};

const CR: u8 = b'\r';
const LF: u8 = b'\n';

fn main() {
    let path = r#"C:\Users\ryanl\Code\prawn\debug.txt"#;
    let file = std::fs::File::options()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)
        .unwrap();
    let mut file = RwLock::new(file);
    writeln!(file.write().unwrap(), "Booting...").unwrap();
    let mut state = State::Header;
    let mut buf = Vec::new();

    let mut stdin = BufReader::new(std::io::stdin());
    let mut stdout = std::io::BufWriter::new(std::io::stdout());
    loop {
        match state {
            State::Header => {
                let amount = stdin.read_until(LF, &mut buf).unwrap();
                if amount == 0 {
                    break;
                }
                let idx = buf.len() - 1;
                if idx >= 3 && buf[idx - 3..=idx] == [CR, LF, CR, LF] {
                    let s = std::str::from_utf8(&buf[..idx - 3]).unwrap();
                    let headers = parse_headers(s);

                    writeln!(file.write().unwrap(), "{:?}", headers).unwrap();

                    let length = headers.get("Content-Length").unwrap().parse().unwrap();
                    buf.clear();
                    buf.resize(length, 0);
                    state = State::Content(length);
                }
            }
            State::Content(len) => {
                debug_assert!(buf.len() == len);
                stdin.read_exact(&mut buf[0..len]).unwrap();
                let s = std::str::from_utf8(&buf).unwrap();
                writeln!(file.write().unwrap(), "{}", s).unwrap();
                let msg = serde_json::from_str::<InitializeRequest>(s);
                if let Ok(msg) = msg {
                    writeln!(file.write().unwrap(), "--> {:#?}", msg).unwrap();
                    let response = initialize(msg);
                    write!(
                        file.write().unwrap(),
                        "<-- {}",
                        serde_json::to_string(&response).unwrap()
                    )
                    .unwrap();
                    write!(stdout, "{}", serde_json::to_string(&response).unwrap()).unwrap();
                }
                buf.clear();
                state = State::Header;
            }
        }
    }
    writeln!(file.write().unwrap(), "Shutting down...").unwrap();
}

fn initialize(request: InitializeRequest) -> InitializeResponse {
    InitializeResponse {
        body: Some(Capabilities::default()),
        success: true,
        seq: request.seq + 1,
        command: String::new(),
        type_: "response".into(),
        request_seq: request.seq,
        message: None,
    }
}

fn parse_headers(header: &str) -> HashMap<String, String> {
    header
        .lines()
        .filter_map(|l| {
            let (n, v) = l.split_once(':')?;
            Some((n.trim().to_owned(), v.trim().to_owned()))
        })
        .collect()
}

#[derive(Debug)]
enum State {
    Header,
    Content(usize),
}
