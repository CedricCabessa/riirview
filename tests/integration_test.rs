use riirview::config::Config;
use riirview::service;
use riirview::{get_connection_pool, run_db_migrations};
use std::env;
use std::fs::File;
use std::io::prelude::*;
use tempfile::NamedTempFile;

#[tokio::test]
async fn test_e2e() {
    unsafe { env::set_var("GH_TOKEN", "faketoken") };

    let mut server = mockito::Server::new_async().await;
    let server_url = server.url();

    let db_file = NamedTempFile::new().unwrap();
    let rule_file = NamedTempFile::new().unwrap();
    Config::init_for_test(
        server_url,
        db_file.path().to_str().unwrap().to_string(),
        rule_file.path().to_str().unwrap().to_string(),
    );

    let pool = get_connection_pool();
    run_db_migrations(&mut pool.get().unwrap());

    let mut file = File::open("tests/notifications.json").unwrap();
    let mut notifications_data = String::new();
    file.read_to_string(&mut notifications_data).unwrap();

    let server_url = server.url();
    server
        .mock(
            "GET",
            mockito::Matcher::Regex(r"^/notifications(\?.*)*$".to_string()),
        )
        .with_header("content-type", "application/json")
        .with_status(200)
        .with_body_from_request(move |_| {
            notifications_data
                .replace("https://api.github.com", &server_url)
                .into()
        })
        .create();

    let mut file = File::open("tests/pulls.json").unwrap();
    let mut pulls_data = String::new();
    file.read_to_string(&mut pulls_data).unwrap();
    let server_url = server.url();
    server
        .mock(
            "GET",
            mockito::Matcher::Regex(r"^/repos/(.*)/(.*)/pulls/(.*)$".to_string()),
        )
        .with_header("content-type", "application/json")
        .with_status(200)
        .with_body_from_request(move |request| {
            let url = format!("{}{}", &server_url, request.path());
            pulls_data
                .replace("REPLACE_URL", &url)
                .replace("https://api.github.com", &server_url)
                .into()
        })
        .create();

    let mut file = File::open("tests/release.json").unwrap();
    let mut release_data = String::new();
    file.read_to_string(&mut release_data).unwrap();
    let server_url = server.url();
    server
        .mock(
            "GET",
            mockito::Matcher::Regex(r"^/repos/(.*)/(.*)/releases/(.*)$".to_string()),
        )
        .with_header("content-type", "application/json")
        .with_status(200)
        .with_body_from_request(move |request| {
            let url = format!("{}{}", &server_url, request.path());
            release_data
                .replace("REPLACE_URL", &url)
                .replace("https://api.github.com", &server_url)
                .into()
        })
        .create();

    let mut file = File::open("tests/issues.json").unwrap();
    let mut issues_data = String::new();
    file.read_to_string(&mut issues_data).unwrap();
    let server_url = server.url();
    server
        .mock(
            "GET",
            mockito::Matcher::Regex(r"^/repos/(.*)/(.*)/issues/(.*)$".to_string()),
        )
        .with_header("content-type", "application/json")
        .with_status(200)
        .with_body_from_request(move |request| {
            let url = format!("{}{}", &server_url, request.path());
            issues_data
                .replace("REPLACE_URL", &url)
                .replace("https://api.github.com", &server_url)
                .into()
        })
        .create();

    service::sync(&mut pool.get().unwrap()).await.unwrap();

    let notifications = service::get_notifications(&mut pool.get().unwrap())
        .await
        .unwrap();
    assert_eq!(notifications.len(), 50);

    //
    // add boost
    //

    let notification = notifications.get(0).unwrap();
    let first_id = notification.id.clone();
    service::update_score(&mut pool.clone().get().unwrap(), notification, 10)
        .await
        .unwrap();
    assert_eq!(notification.score_boost, 0); // not updated yet

    let notifications = service::get_notifications(&mut pool.get().unwrap())
        .await
        .unwrap();
    let notification = notifications.get(0).unwrap();
    assert_eq!(notification.id, first_id);
    assert_eq!(notification.score_boost, 10); // updated

    // resync
    service::sync(&mut pool.get().unwrap()).await.unwrap();

    let notifications = service::get_notifications(&mut pool.get().unwrap())
        .await
        .unwrap();
    assert_eq!(notifications.len(), 50);
    let notification = notifications.get(0).unwrap();
    assert_eq!(notification.id, first_id);
    assert_eq!(notification.score_boost, 10);
}
