use fo3_server::blaze::server::{run_test_client, BlazeServer};
use fo3_server::blaze::tdf::decode_tdf;
use fo3_server::blaze::{
    build_frame, frames_and_rest, CMD_PING, COMP_UTIL, MSG_MESSAGE, MSG_REPLY,
};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

#[test]
fn test_blaze_loopback_full_exchange() {
    let test_dir = std::env::temp_dir().join(format!(
        "fo3_blaze_test_cap_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    // Bind on port 0 for ephemeral ports
    let server = BlazeServer::bind(0, 0, test_dir.clone())
        .expect("Failed to bind BlazeServer on ephemeral ports");
    let redir_port = server.redir_port;
    let noti_port = server.noti_port;
    let state = server.state.clone();

    assert_ne!(redir_port, 0);
    assert_ne!(noti_port, 0);

    let running = Arc::new(AtomicBool::new(true));
    let running_server = server
        .start(running.clone())
        .expect("Failed to start BlazeServer");

    // Execute the 8-command client exchange
    let result = run_test_client("127.0.0.1", redir_port);
    assert!(
        result.is_ok(),
        "Blaze loopback client failed: {:?}",
        result.err()
    );

    // Verify session management recorded the session
    assert_eq!(state.session_manager.active_count(), 1);
    let session = state
        .session_manager
        .get_session("LOCAL-SESSION-0001")
        .expect("Session LOCAL-SESSION-0001 should exist");
    assert_eq!(session.display_name, "player");
    assert_eq!(session.user_id, 1);

    // Verify game manager recorded the game
    assert!(state.game_manager.last_game_id() >= 1000);

    // Verify packet capture files were written
    let entries = std::fs::read_dir(&test_dir).expect("Capture directory should exist");
    let mut cap_files = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) == Some("bin") {
            let meta = std::fs::metadata(&p).unwrap();
            if meta.len() > 0 {
                cap_files.push(p);
            }
        }
    }
    assert!(
        !cap_files.is_empty(),
        "Expected at least one non-empty capture file in {:?}",
        test_dir
    );

    // Stop server
    running_server.stop();
    std::thread::sleep(Duration::from_millis(50));
    running_server.join();

    // Clean up
    let _ = std::fs::remove_dir_all(&test_dir);
}

#[test]
fn test_blaze_notification_port_ping() {
    let test_dir = std::env::temp_dir().join(format!(
        "fo3_blaze_noti_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    let server = BlazeServer::bind(0, 0, test_dir.clone())
        .expect("Failed to bind BlazeServer on ephemeral ports");
    let noti_port = server.noti_port;

    let running = Arc::new(AtomicBool::new(true));
    let running_server = server
        .start(running.clone())
        .expect("Failed to start BlazeServer");

    // Connect to notification port and send a ping
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", noti_port))
        .expect("Failed to connect to notification port");
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();

    let ping_req = build_frame(COMP_UTIL, CMD_PING, MSG_MESSAGE, 99, &[], 0);
    stream.write_all(&ping_req).unwrap();

    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf).unwrap();
    assert!(n > 0, "Expected ping reply on notification port");

    let (frames, _) = frames_and_rest(&buf[..n]);
    assert_eq!(frames.len(), 1);
    let (hdr, payload) = &frames[0];
    assert_eq!(hdr.component, COMP_UTIL);
    assert_eq!(hdr.command, CMD_PING);
    assert_eq!(hdr.msgtype, MSG_REPLY);
    assert_eq!(hdr.msgid, 99);

    let fields = decode_tdf(payload);
    assert!(
        fields.iter().any(|(k, _)| k == "STIM"),
        "STIM expected in ping reply"
    );

    // Stop server
    running_server.stop();
    std::thread::sleep(Duration::from_millis(50));
    running_server.join();

    let _ = std::fs::remove_dir_all(&test_dir);
}

#[test]
fn test_blaze_multiple_client_sessions() {
    let test_dir = std::env::temp_dir().join(format!(
        "fo3_blaze_multi_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    let server = BlazeServer::bind(0, 0, test_dir.clone())
        .expect("Failed to bind BlazeServer on ephemeral ports");
    let redir_port = server.redir_port;
    let state = server.state.clone();

    let running = Arc::new(AtomicBool::new(true));
    let running_server = server
        .start(running.clone())
        .expect("Failed to start BlazeServer");

    // First client run
    let r1 = run_test_client("127.0.0.1", redir_port);
    assert!(r1.is_ok(), "Client 1 failed: {:?}", r1.err());

    // Second client run (reconnecting with same user)
    let r2 = run_test_client("127.0.0.1", redir_port);
    assert!(r2.is_ok(), "Client 2 failed: {:?}", r2.err());

    // Connect a second user with distinct credentials
    let mut stream2 = TcpStream::connect(format!("127.0.0.1:{}", redir_port))
        .expect("Failed to connect second client");
    stream2.set_read_timeout(Some(Duration::from_secs(3))).unwrap();

    let mut login_body = Vec::new();
    login_body.extend_from_slice(&fo3_server::blaze::tdf::enc_string("MAIL", "user2@local"));
    login_body.extend_from_slice(&fo3_server::blaze::tdf::enc_string("PASS", "secret"));
    let req = build_frame(
        fo3_server::blaze::COMP_AUTHENTICATION,
        fo3_server::blaze::CMD_LOGIN,
        MSG_MESSAGE,
        10,
        &login_body,
        0,
    );
    stream2.write_all(&req).unwrap();

    let mut buf = [0u8; 1024];
    let n = stream2.read(&mut buf).unwrap();
    let (frames, _) = frames_and_rest(&buf[..n]);
    assert_eq!(frames.len(), 1);
    let fields = decode_tdf(&frames[0].1);
    let skey = fo3_server::blaze::server::find_str(&fields, "SKEY");
    assert_eq!(skey, Some("LOCAL-SESSION-0002"));

    // Session manager should now have recorded 2 unique sessions
    assert_eq!(state.session_manager.active_count(), 2);
    assert!(state.session_manager.get_session("LOCAL-SESSION-0001").is_some());
    assert!(state.session_manager.get_session("LOCAL-SESSION-0002").is_some());

    // Stop server
    running_server.stop();
    std::thread::sleep(Duration::from_millis(50));
    running_server.join();

    let _ = std::fs::remove_dir_all(&test_dir);
}
