use debugserver_types::{Capabilities, InitializeRequest, InitializeResponse, InitializedEvent};
use fd_lock::RwLock;
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufRead, BufReader, BufWriter, Read, Stdout, Write},
};

const CR: u8 = b'\r';
const LF: u8 = b'\n';

fn main() {
    let path = r#"D:\Code\prawn\debug.txt"#;
    let mut emitter = Emitter::new(path);
    emitter.log("Booting up...");

    let mut state = State::Header;
    let mut buf = Vec::new();

    let mut stdin = BufReader::new(io::stdin());
    let mut sequence = SequenceNumber::new();
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

                    emitter.log(&format!("{headers:?}"));

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

                let req = serde_json::from_str::<Request>(s).unwrap();
                match req.command {
                    CommandKind::Initialize => {
                        let msg = serde_json::from_str::<InitializeRequest>(s);
                        if let Ok(msg) = msg {
                            handle_initialize(&mut emitter, msg, &mut sequence);
                        }
                    }
                    CommandKind::Disconnect => {
                        emitter.log("Shutting down");
                        break;
                    }
                }

                buf.clear();
                state = State::Header;
            }
        }
    }
}

#[derive(Deserialize)]
pub struct Request {
    command: CommandKind,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum CommandKind {
    Initialize,
    Disconnect,
}

fn handle_initialize(emitter: &mut Emitter, msg: InitializeRequest, sequence: &mut SequenceNumber) {
    emitter.log_incoming(&format!("{msg:#?}"));
    // Send the "initialize response"
    let response = initialize(msg, sequence);
    emitter.send(response);
    // Send the "initialize event"
    let event = InitializedEvent {
        event: "initialized".into(),
        type_: "event".into(),
        seq: sequence.next(),
        body: None,
    };
    emitter.send(event);
}

/// A wrapper around stdio which also logs to a file.
pub struct Emitter {
    log_file: RwLock<File>,
    stdout: BufWriter<Stdout>,
}

impl Emitter {
    pub fn new(path: &str) -> Self {
        let log_file = File::options()
            .create(true)
            .write(true)
            .truncate(false)
            .open(path)
            .unwrap();

        let log_file = RwLock::new(log_file);
        let stdout = BufWriter::new(io::stdout());

        Self { log_file, stdout }
    }

    pub fn send<T>(&mut self, response: T)
    where
        T: serde::Serialize,
    {
        let body = serde_json::to_string(&response).unwrap();
        let headers = format!("Content-Length: {}", body.len());
        writeln!(self.log_file.write().unwrap(), "[send] {headers}\n{body}").unwrap();
        writeln!(self.stdout, "{headers}\r\n\r\n{body}").unwrap();
    }

    pub fn log(&mut self, s: &str) {
        writeln!(self.log_file.write().unwrap(), "{s}",).unwrap();
    }

    pub fn log_incoming(&mut self, s: &str) {
        writeln!(self.log_file.write().unwrap(), "[recv] {s}",).unwrap();
    }
}

fn initialize(request: InitializeRequest, seq: &mut SequenceNumber) -> InitializeResponse {
    let c = Capabilities {
        additional_module_columns: None,
        exception_breakpoint_filters: None,
        support_terminate_debuggee: Some(true),
        supported_checksum_algorithms: None,
        supports_completions_request: Some(true),
        supports_conditional_breakpoints: Some(true),
        supports_configuration_done_request: Some(true),
        supports_data_breakpoints: Some(true),
        supports_delayed_stack_trace_loading: Some(true),
        supports_evaluate_for_hovers: Some(true),
        supports_exception_info_request: Some(true),
        supports_exception_options: Some(true),
        supports_function_breakpoints: Some(true),
        supports_goto_targets_request: Some(true),
        supports_hit_conditional_breakpoints: Some(true),
        supports_loaded_sources_request: Some(true),
        supports_log_points: Some(true),
        supports_modules_request: Some(true),
        supports_restart_frame: Some(true),
        supports_restart_request: Some(true),
        supports_set_expression: Some(true),
        supports_set_variable: Some(true),
        supports_step_back: Some(true),
        supports_step_in_targets_request: Some(true),
        supports_terminate_request: Some(true),
        supports_terminate_threads_request: Some(true),
        supports_value_formatting_options: Some(true),
    };
    InitializeResponse {
        body: Some(c),
        success: true,
        seq: request.seq,
        command: "initialize".into(),
        type_: "response".into(),
        request_seq: seq.next(),
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

/// Monotonically increasing sequence number.
struct SequenceNumber {
    counter: i64,
}

impl SequenceNumber {
    /// Create a new instance.
    pub(crate) fn new() -> Self {
        Self { counter: 0 }
    }

    pub(crate) fn next(&mut self) -> i64 {
        self.counter += 1;
        self.counter
    }
}
