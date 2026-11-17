//! Synthetic loopback proof with a host and two separate client processes.
//! This demonstrates local role-token routing only; it is not production authentication.
use grapevine_workbench::{
    game_sessions::{Attendance, SessionQuote, SessionRequest},
    model::{Role, Summary},
    store::{Library, SessionReceipt},
    xp::Baseline,
};
use serde::{Deserialize, Serialize};
use std::{
    env,
    io::{self, BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

const MAX_FRAME: usize = 64 * 1024;
const SOCKET_TIMEOUT: Duration = Duration::from_secs(2);
const SUPERVISOR_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Serialize, Deserialize)]
struct Request {
    token: String,
    op: String,
    #[serde(default)]
    expected_revision: Option<i64>,
    #[serde(default)]
    session_request: Option<SessionRequest>,
    #[serde(default)]
    quote: Option<SessionQuote>,
    #[serde(default)]
    character_id: Option<String>,
    // Deliberately ignored by the host. Authorization comes only from the token map.
    #[serde(default)]
    claimed_role: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
struct Response {
    ok: bool,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    revision: Option<i64>,
    #[serde(default)]
    summaries: Vec<Summary>,
    #[serde(default)]
    quote: Option<SessionQuote>,
    #[serde(default)]
    receipt: Option<SessionReceipt>,
    #[serde(default)]
    receipts: Vec<SessionReceipt>,
    #[serde(default)]
    xp: Option<XpSnapshot>,
}

#[derive(Serialize, Deserialize)]
struct XpSnapshot {
    earned: Option<String>,
    unspent: Option<String>,
    ledger_entries: usize,
}

#[derive(Serialize, Deserialize)]
struct CommitResult {
    ok: bool,
    code: Option<String>,
    revision: Option<i64>,
    committed_revision: Option<i64>,
}

struct Tokens {
    storyteller: String,
    player: String,
    shutdown: String,
}

fn main() {
    let result = match env::var("GV_TWO_MODE").as_deref() {
        Ok("host") => host_main(),
        Ok("client") => client_main(),
        _ => supervisor_main(),
    };
    if result.is_err() {
        eprintln!("Synthetic two-client validation failed.");
        std::process::exit(1);
    }
}

fn env_value(name: &str) -> Result<String, String> {
    env::var(name).map_err(|_| "required process setting is missing".to_owned())
}

fn hidden(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    #[cfg(not(windows))]
    let _ = command;
}

fn host_main() -> Result<(), String> {
    let path = env_value("GV_TWO_DB")?;
    let tokens = Tokens {
        storyteller: env_value("GV_TWO_ST_TOKEN")?,
        player: env_value("GV_TWO_PLAYER_TOKEN")?,
        shutdown: env_value("GV_TWO_SHUTDOWN_TOKEN")?,
    };
    let mut library = Library::open(std::path::Path::new(&path))
        .map_err(|_| "host could not open synthetic library")?;
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|_| "loopback bind failed")?;
    listener
        .set_nonblocking(true)
        .map_err(|_| "listener setup failed")?;
    println!(
        "READY {}",
        listener
            .local_addr()
            .map_err(|_| "listener address failed")?
    );
    io::stdout()
        .flush()
        .map_err(|_| "host readiness flush failed")?;

    loop {
        match listener.accept() {
            Ok((mut stream, _peer)) => {
                let _ = stream.set_read_timeout(Some(SOCKET_TIMEOUT));
                let _ = stream.set_write_timeout(Some(SOCKET_TIMEOUT));
                let frame = match read_frame(&mut stream) {
                    Ok(Some(frame)) => frame,
                    _ => continue,
                };
                let request: Request = match serde_json::from_slice(&frame) {
                    Ok(request) => request,
                    Err(_) => continue,
                };
                if request.op == "shutdown" {
                    let response = if request.token == tokens.shutdown {
                        ok_response(library.revision().ok())
                    } else {
                        failure("UNAUTHORIZED", None)
                    };
                    let _ = write_frame(&mut stream, &response);
                    if request.token == tokens.shutdown {
                        break;
                    }
                    continue;
                }
                let response = dispatch(&mut library, request, &tokens);
                let _ = write_frame(&mut stream, &response);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(8));
            }
            Err(_) => thread::sleep(Duration::from_millis(8)),
        }
    }
    Ok(())
}

fn dispatch(library: &mut Library, request: Request, tokens: &Tokens) -> Response {
    let role = if request.token == tokens.storyteller {
        Some(Role::Storyteller)
    } else if request.token == tokens.player {
        Some(Role::Player)
    } else {
        None
    };
    let Some(role) = role else {
        return failure("UNAUTHORIZED", None);
    };
    let revision = library.revision().ok();

    match request.op.as_str() {
        "summaries" => {
            if role != Role::Player {
                return failure("FORBIDDEN", revision);
            }
            match library.summaries(Role::Player) {
                Ok(summaries) => Response {
                    ok: true,
                    revision,
                    summaries,
                    ..Response::default()
                },
                Err(error) => failure(error.code, revision),
            }
        }
        "quote" => {
            if role != Role::Storyteller {
                return failure("FORBIDDEN", revision);
            }
            let (Some(request), Some(expected)) =
                (request.session_request.as_ref(), request.expected_revision)
            else {
                return failure("BAD_REQUEST", revision);
            };
            match library.quote_session(request, expected, Role::Storyteller) {
                Ok(quote) => Response {
                    ok: true,
                    revision,
                    quote: Some(quote),
                    ..Response::default()
                },
                Err(error) => failure(error.code, revision),
            }
        }
        "commit" => {
            // Reject before calling the private write service; claimed_role is ignored.
            if role != Role::Storyteller {
                return failure("FORBIDDEN", revision);
            }
            let Some(quote) = request.quote.as_ref() else {
                return failure("BAD_REQUEST", revision);
            };
            match library.commit_session(quote, Role::Storyteller) {
                Ok(receipt) => Response {
                    ok: true,
                    revision: library.revision().ok(),
                    receipt: Some(receipt),
                    ..Response::default()
                },
                Err(error) => failure(error.code, library.revision().ok()),
            }
        }
        "receipts" => {
            if role != Role::Storyteller {
                return failure("FORBIDDEN", revision);
            }
            match library.session_receipts(Role::Storyteller) {
                Ok(receipts) => Response {
                    ok: true,
                    revision,
                    receipts,
                    ..Response::default()
                },
                Err(error) => failure(error.code, revision),
            }
        }
        "xp_view" => {
            if role != Role::Storyteller {
                return failure("FORBIDDEN", revision);
            }
            let Some(id) = request.character_id.as_deref() else {
                return failure("BAD_REQUEST", revision);
            };
            match library.xp_view(id, Role::Storyteller) {
                Ok(view) => Response {
                    ok: true,
                    revision,
                    xp: Some(XpSnapshot {
                        earned: view.earned,
                        unspent: view.unspent,
                        ledger_entries: view.ledger.map_or(0, |ledger| ledger.entries.len()),
                    }),
                    ..Response::default()
                },
                Err(error) => failure(error.code, revision),
            }
        }
        _ => failure("BAD_REQUEST", revision),
    }
}

fn ok_response(revision: Option<i64>) -> Response {
    Response {
        ok: true,
        revision,
        ..Response::default()
    }
}

fn failure(code: &str, revision: Option<i64>) -> Response {
    Response {
        ok: false,
        code: Some(code.to_owned()),
        revision,
        ..Response::default()
    }
}

fn read_frame(stream: &mut TcpStream) -> io::Result<Option<Vec<u8>>> {
    let mut frame = Vec::new();
    let mut chunk = [0_u8; 2048];
    loop {
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            return Ok(None);
        }
        if let Some(newline) = chunk[..count].iter().position(|byte| *byte == b'\n') {
            if frame.len() + newline > MAX_FRAME {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "frame limit"));
            }
            frame.extend_from_slice(&chunk[..newline]);
            return Ok(Some(frame));
        }
        if frame.len() + count > MAX_FRAME {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "frame limit"));
        }
        frame.extend_from_slice(&chunk[..count]);
    }
}

fn write_frame(stream: &mut TcpStream, response: &Response) -> io::Result<()> {
    let mut frame = serde_json::to_vec(response)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "encode response"))?;
    if frame.len() > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "response limit"));
    }
    frame.push(b'\n');
    stream.write_all(&frame)
}

fn connect(address: &str) -> Result<TcpStream, String> {
    let address: SocketAddr = address.parse().map_err(|_| "invalid loopback address")?;
    let stream = TcpStream::connect_timeout(&address, SOCKET_TIMEOUT)
        .map_err(|_| "loopback connection failed")?;
    stream
        .set_read_timeout(Some(SOCKET_TIMEOUT))
        .map_err(|_| "client timeout setup failed")?;
    stream
        .set_write_timeout(Some(SOCKET_TIMEOUT))
        .map_err(|_| "client timeout setup failed")?;
    Ok(stream)
}

fn round_trip(address: &str, request: &Request) -> Result<Response, String> {
    let mut stream = connect(address)?;
    let mut frame = serde_json::to_vec(request).map_err(|_| "request encode failed")?;
    if frame.len() > MAX_FRAME {
        return Err("request exceeds protocol limit".into());
    }
    frame.push(b'\n');
    stream
        .write_all(&frame)
        .map_err(|_| "request write failed")?;
    let response = read_frame(&mut stream)
        .map_err(|_| "response read failed")?
        .ok_or("host closed without a response")?;
    serde_json::from_slice(&response).map_err(|_| "response decode failed".into())
}

fn client_main() -> Result<(), String> {
    let address = env_value("GV_TWO_ADDRESS")?;
    let token = env_value("GV_TWO_CLIENT_TOKEN")?;
    let request: SessionRequest = serde_json::from_str(&env_value("GV_TWO_SESSION")?)
        .map_err(|_| "client session request could not be decoded")?;
    let expected = env_value("GV_TWO_REVISION")?
        .parse::<i64>()
        .map_err(|_| "client revision could not be decoded")?;
    let role_hint = env_value("GV_TWO_ROLE_HINT").unwrap_or_else(|_| "Storyteller".into());
    let quote_reply = round_trip(
        &address,
        &Request {
            token: token.clone(),
            op: "quote".into(),
            expected_revision: Some(expected),
            session_request: Some(request),
            claimed_role: Some(role_hint.clone()),
            ..empty_request()
        },
    )?;
    let Some(quote) = quote_reply.quote else {
        println!(
            "QUOTE_ERROR {}",
            quote_reply.code.as_deref().unwrap_or("FAILED")
        );
        return Err("client quote was rejected".into());
    };
    println!("QUOTED");
    io::stdout().flush().map_err(|_| "client output failed")?;
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let action = line.map_err(|_| "client gate read failed")?;
        let operation = match action.trim() {
            "COMMIT" | "RETRY" => "RESULT",
            "EXIT" => break,
            _ => continue,
        };
        let response = round_trip(
            &address,
            &Request {
                token: token.clone(),
                op: "commit".into(),
                quote: Some(quote.clone()),
                claimed_role: Some(role_hint.clone()),
                ..empty_request()
            },
        )?;
        let result = CommitResult {
            ok: response.ok,
            code: response.code,
            revision: response.revision,
            committed_revision: response.receipt.map(|receipt| receipt.committed_revision),
        };
        println!(
            "{operation} {}",
            serde_json::to_string(&result).map_err(|_| "result encode failed")?
        );
        io::stdout().flush().map_err(|_| "client output failed")?;
    }
    Ok(())
}

fn empty_request() -> Request {
    Request {
        token: String::new(),
        op: String::new(),
        expected_revision: None,
        session_request: None,
        quote: None,
        character_id: None,
        claimed_role: None,
    }
}

struct ChildGroup {
    children: Vec<Child>,
}

impl ChildGroup {
    fn new() -> Self {
        Self {
            children: Vec::new(),
        }
    }

    fn spawn(
        &mut self,
        command: &mut Command,
        with_stdin: bool,
    ) -> Result<(usize, Receiver<String>, Option<ChildStdin>), String> {
        command
            .stdin(if with_stdin {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        hidden(command);
        let mut child = command
            .spawn()
            .map_err(|_| "child process could not start")?;
        let stdout = child.stdout.take().ok_or("child output pipe unavailable")?;
        let stdin = child.stdin.take();
        let index = self.children.len();
        self.children.push(child);
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) => {
                        if tx.send(line.trim_end().to_owned()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        Ok((index, rx, stdin))
    }

    fn wait(&mut self, index: usize, deadline: Instant) -> Result<(), String> {
        loop {
            if self.children[index]
                .try_wait()
                .map_err(|_| "child process status failed")?
                .is_some()
            {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err("child process exceeded supervisor deadline".into());
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn terminate_all(&mut self) {
        for child in &mut self.children {
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
            }
            let _ = child.wait();
        }
    }
}

impl Drop for ChildGroup {
    fn drop(&mut self) {
        self.terminate_all();
    }
}

struct RaceClient {
    child_index: usize,
    input: ChildStdin,
    output: Receiver<String>,
}

fn receive_line(receiver: &Receiver<String>, deadline: Instant) -> Result<String, String> {
    let left = deadline.saturating_duration_since(Instant::now());
    if left.is_zero() {
        return Err("supervisor deadline expired".into());
    }
    receiver
        .recv_timeout(left.min(SOCKET_TIMEOUT))
        .map_err(|_| "child output timed out".into())
}

fn receive_prefixed(
    receiver: &Receiver<String>,
    prefix: &str,
    deadline: Instant,
) -> Result<String, String> {
    let line = receive_line(receiver, deadline)?;
    if let Some(rest) = line.strip_prefix(prefix) {
        return Ok(rest.trim().to_owned());
    }
    Err("child returned an unexpected status".into())
}

fn request(token: &str, op: &str) -> Request {
    Request {
        token: token.to_owned(),
        op: op.to_owned(),
        expected_revision: None,
        session_request: None,
        quote: None,
        character_id: None,
        claimed_role: None,
    }
}

fn launch_host(
    group: &mut ChildGroup,
    path: &str,
    tokens: &Tokens,
    deadline: Instant,
) -> Result<(usize, String), String> {
    let mut command = Command::new(env::current_exe().map_err(|_| "executable path unavailable")?);
    command
        .env("GV_TWO_MODE", "host")
        .env("GV_TWO_DB", path)
        .env("GV_TWO_ST_TOKEN", &tokens.storyteller)
        .env("GV_TWO_PLAYER_TOKEN", &tokens.player)
        .env("GV_TWO_SHUTDOWN_TOKEN", &tokens.shutdown);
    let (index, output, _) = group.spawn(&mut command, false)?;
    let ready = receive_line(&output, deadline)?;
    let address = ready
        .strip_prefix("READY ")
        .ok_or("host readiness was not received")?
        .trim()
        .to_owned();
    address
        .parse::<SocketAddr>()
        .map_err(|_| "host address was invalid")?;
    Ok((index, address))
}

fn spawn_race_client(
    group: &mut ChildGroup,
    address: &str,
    token: &str,
    revision: i64,
    session: &SessionRequest,
) -> Result<RaceClient, String> {
    let mut command = Command::new(env::current_exe().map_err(|_| "executable path unavailable")?);
    command
        .env("GV_TWO_MODE", "client")
        .env("GV_TWO_ADDRESS", address)
        .env("GV_TWO_CLIENT_TOKEN", token)
        .env(
            "GV_TWO_SESSION",
            serde_json::to_string(session).map_err(|_| "session encode failed")?,
        )
        .env("GV_TWO_REVISION", revision.to_string())
        .env("GV_TWO_ROLE_HINT", "Storyteller");
    let (child_index, output, input) = group.spawn(&mut command, true)?;
    Ok(RaceClient {
        child_index,
        input: input.ok_or("client input pipe unavailable")?,
        output,
    })
}

fn tell(client: &mut RaceClient, text: &str) -> Result<(), String> {
    writeln!(client.input, "{text}").map_err(|_| "client control write failed")?;
    client
        .input
        .flush()
        .map_err(|_| "client control flush failed".to_string())
}

fn direct(address: &str, request: &Request) -> Result<Response, String> {
    round_trip(address, request)
}

fn quote_request(token: &str, expected: i64, session: &SessionRequest) -> Request {
    Request {
        token: token.to_owned(),
        op: "quote".into(),
        expected_revision: Some(expected),
        session_request: Some(session.clone()),
        claimed_role: Some("Storyteller".into()),
        ..empty_request()
    }
}

fn supervisor_main() -> Result<(), String> {
    let deadline = Instant::now() + SUPERVISOR_TIMEOUT;
    let temp = tempfile::TempDir::new().map_err(|_| "synthetic workspace unavailable")?;
    let db = temp.path().join("synthetic.gvlib");
    let mut library = Library::create(&db, "Synthetic two-client proof")
        .map_err(|_| "synthetic library creation failed")?;
    let character_id = library
        .create_character(
            "Vampire",
            "Synthetic vampire",
            library.revision().map_err(|_| "revision read failed")?,
            Role::Storyteller,
        )
        .map_err(|_| "synthetic character creation failed")?;
    library
        .initialize_xp(
            &character_id,
            &Baseline {
                day: "2026-01-01".into(),
                earned: "10".into(),
                unspent: "10".into(),
                actor: "Synthetic operator".into(),
                reason: "Synthetic opening balance".into(),
                evidence: "Synthetic fixture".into(),
            },
            library.revision().map_err(|_| "revision read failed")?,
            Role::Storyteller,
        )
        .map_err(|_| "synthetic XP opening failed")?;
    let initial_revision = library.revision().map_err(|_| "revision read failed")?;
    drop(library);

    let tokens = Tokens {
        storyteller: uuid::Uuid::new_v4().to_string(),
        player: uuid::Uuid::new_v4().to_string(),
        shutdown: uuid::Uuid::new_v4().to_string(),
    };
    let sessions = ["2", "3"].map(|amount| SessionRequest {
        id: uuid::Uuid::new_v4().to_string(),
        title: "Synthetic loopback race".into(),
        day: "2026-01-02".into(),
        actor: "Synthetic operator".into(),
        reason: "Manually entered synthetic award".into(),
        evidence: "Synthetic attendance".into(),
        attendance: vec![Attendance {
            record_id: character_id.clone(),
            present: true,
            award: Some(amount.into()),
        }],
    });
    let mut children = ChildGroup::new();
    let db_string = db.to_string_lossy().into_owned();
    let (host_index, address) = launch_host(&mut children, &db_string, &tokens, deadline)?;

    let player_summaries = direct(&address, &request(&tokens.player, "summaries"))?;
    if !player_summaries.ok
        || !player_summaries
            .summaries
            .iter()
            .any(|summary| summary.id == character_id && summary.kind == "Vampire")
    {
        return Err("Player summary request failed".into());
    }

    let mut clients = [
        spawn_race_client(
            &mut children,
            &address,
            &tokens.storyteller,
            initial_revision,
            &sessions[0],
        )?,
        spawn_race_client(
            &mut children,
            &address,
            &tokens.storyteller,
            initial_revision,
            &sessions[1],
        )?,
    ];
    for client in &clients {
        if receive_line(&client.output, deadline)? != "QUOTED" {
            return Err("a race client could not quote".into());
        }
    }
    let mut quotes = Vec::new();
    for session in &sessions {
        let response = direct(
            &address,
            &quote_request(&tokens.storyteller, initial_revision, session),
        )?;
        if !response.ok {
            return Err("host quote verification failed".into());
        }
        quotes.push(response.quote.ok_or("host quote was missing")?);
    }
    for client in &mut clients {
        tell(client, "COMMIT")?;
    }
    let first: CommitResult =
        serde_json::from_str(&receive_prefixed(&clients[0].output, "RESULT ", deadline)?)
            .map_err(|_| "first race result could not be decoded")?;
    let second: CommitResult =
        serde_json::from_str(&receive_prefixed(&clients[1].output, "RESULT ", deadline)?)
            .map_err(|_| "second race result could not be decoded")?;
    let results = [first, second];
    let winners = results.iter().filter(|result| result.ok).count();
    let losers = results
        .iter()
        .filter(|result| !result.ok && result.code.as_deref() == Some("STALE_REVISION"))
        .count();
    if winners != 1 || losers != 1 {
        return Err("concurrent commit did not produce one winner and one stale client".into());
    }
    let winner = usize::from(!results[0].ok);
    let winner_revision = results[winner]
        .committed_revision
        .ok_or("winning commit revision was missing")?;
    if winner_revision != initial_revision + 1 {
        return Err("winning commit advanced an unexpected revision".into());
    }

    tell(&mut clients[winner], "RETRY")?;
    let retry: CommitResult = serde_json::from_str(&receive_prefixed(
        &clients[winner].output,
        "RESULT ",
        deadline,
    )?)
    .map_err(|_| "idempotent retry result could not be decoded")?;
    if !retry.ok || retry.committed_revision != Some(winner_revision) {
        return Err("exact winner retry was not idempotent".into());
    }

    let wrong_token = uuid::Uuid::new_v4().to_string();
    let mut wrong_request = quote_request(&wrong_token, initial_revision, &sessions[0]);
    wrong_request.quote = Some(quotes[0].clone());
    wrong_request.op = "commit".into();
    wrong_request.session_request = None;
    wrong_request.expected_revision = None;
    let wrong = direct(&address, &wrong_request)?;
    if wrong.ok
        || wrong.code.as_deref() != Some("UNAUTHORIZED")
        || wrong.revision.is_some()
        || wrong.quote.is_some()
        || !wrong.receipts.is_empty()
        || !wrong.summaries.is_empty()
    {
        return Err("unknown token was not rejected".into());
    }
    let mut forged_player = request(&tokens.player, "commit");
    forged_player.claimed_role = Some("Storyteller".into());
    forged_player.quote = Some(quotes[winner].clone());
    let forged = direct(&address, &forged_player)?;
    if forged.ok || forged.code.as_deref() != Some("FORBIDDEN") {
        return Err("Player token with forged role was not rejected before write".into());
    }

    let before_disconnect = direct(&address, &request(&tokens.storyteller, "receipts"))?;
    let receipt_count_before = before_disconnect.receipts.len();
    let revision_before_disconnect = before_disconnect.revision.ok_or("revision was missing")?;
    let mut partial = connect(&address)?;
    partial
        .write_all(b"{\"token\":\"partial")
        .map_err(|_| "partial-disconnect setup failed")?;
    drop(partial);
    let after_disconnect = direct(&address, &request(&tokens.storyteller, "receipts"))?;
    if after_disconnect.receipts.len() != receipt_count_before
        || after_disconnect.revision != Some(revision_before_disconnect)
    {
        return Err("partial disconnect changed library state".into());
    }

    for client in &mut clients {
        tell(client, "EXIT")?;
        children.wait(client.child_index, deadline)?;
    }
    shutdown_host(&address, &tokens.shutdown)?;
    children.wait(host_index, deadline)?;

    let (restarted_index, restarted_address) =
        launch_host(&mut children, &db_string, &tokens, deadline)?;
    let durable_receipts = direct(
        &restarted_address,
        &request(&tokens.storyteller, "receipts"),
    )?;
    let durable_xp = direct(
        &restarted_address,
        &Request {
            character_id: Some(character_id),
            ..request(&tokens.storyteller, "xp_view")
        },
    )?;
    let expected_amount = if winner == 0 { "12" } else { "13" };
    let xp = durable_xp.xp.ok_or("durable XP summary was missing")?;
    if !durable_receipts.ok
        || durable_receipts.receipts.len() != 1
        || durable_receipts.receipts[0].quote != quotes[winner]
        || durable_receipts.revision != Some(winner_revision)
        || !durable_xp.ok
        || xp.earned.as_deref() != Some(expected_amount)
        || xp.unspent.as_deref() != Some(expected_amount)
        || xp.ledger_entries != 1
    {
        return Err("restarted host did not retain exactly one session award".into());
    }
    shutdown_host(&restarted_address, &tokens.shutdown)?;
    children.wait(restarted_index, deadline)?;
    println!(
        "PASS: two child clients raced; one commit, stale loser, idempotent retry, auth rejection, disconnect safety, and restart durability verified."
    );
    Ok(())
}

fn shutdown_host(address: &str, token: &str) -> Result<(), String> {
    let response = direct(address, &request(token, "shutdown"))?;
    if response.ok {
        Ok(())
    } else {
        Err("host shutdown was rejected".into())
    }
}
