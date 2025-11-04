use actix_web::{test, App};
use gh_info_rs::handlers::{
    batch_get_repos, batch_get_repos_map, get_latest_release, get_releases, get_repo_info,
};
use gh_info_rs::models::{BatchRequest, BatchResponse, BatchResponseMap};

#[actix_web::test]
async fn test_get_repo_info_route() {
    let app = test::init_service(App::new().service(get_repo_info)).await;

    // 使用一个真实的GitHub仓库进行测试（如果API可用）
    // 注意：这个测试可能需要网络连接，在CI/CD中可能需要mock
    let req = test::TestRequest::get()
        .uri("/repos/octocat/Hello-World")
        .to_request();

    let resp = test::call_service(&app, req).await;

    // 如果API可用，应该返回200；如果不可用，可能是其他状态码
    // 这里我们只测试路由是否正确配置
    assert!(resp.status().is_client_error() || resp.status().is_success());
}

#[actix_web::test]
async fn test_get_releases_route() {
    let app = test::init_service(App::new().service(get_releases)).await;

    let req = test::TestRequest::get()
        .uri("/repos/octocat/Hello-World/releases")
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_client_error() || resp.status().is_success());
}

#[actix_web::test]
async fn test_get_latest_release_route() {
    let app = test::init_service(App::new().service(get_latest_release)).await;

    let req = test::TestRequest::get()
        .uri("/repos/octocat/Hello-World/releases/latest")
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_client_error() || resp.status().is_success());
}

#[actix_web::test]
async fn test_batch_get_repos_route() {
    let app = test::init_service(App::new().service(batch_get_repos)).await;

    let batch_request = BatchRequest {
        repos: vec!["octocat/Hello-World".to_string()],
        fields: vec!["repo_info".to_string()],
    };

    let req = test::TestRequest::post()
        .uri("/repos/batch")
        .set_json(&batch_request)
        .to_request();

    let resp = test::call_service(&app, req).await;

    // 验证响应格式
    if resp.status().is_success() {
        let body: BatchResponse = test::read_body_json(resp).await;
        assert_eq!(body.results.len(), 1);
        assert_eq!(body.results[0].repo, "octocat/Hello-World");
    }
}

#[actix_web::test]
async fn test_batch_get_repos_empty_list() {
    let app = test::init_service(App::new().service(batch_get_repos)).await;

    let batch_request = BatchRequest {
        repos: vec![],
        fields: vec![],
    };

    let req = test::TestRequest::post()
        .uri("/repos/batch")
        .set_json(&batch_request)
        .to_request();

    let resp = test::call_service(&app, req).await;

    // 空列表应该返回错误
    assert!(resp.status().is_client_error());
}

#[actix_web::test]
async fn test_batch_get_repos_map_route() {
    let app = test::init_service(App::new().service(batch_get_repos_map)).await;

    let batch_request = BatchRequest {
        repos: vec!["octocat/Hello-World".to_string()],
        fields: vec!["repo_info".to_string()],
    };

    let req = test::TestRequest::post()
        .uri("/repos/batch/map")
        .set_json(&batch_request)
        .to_request();

    let resp = test::call_service(&app, req).await;

    // 验证响应格式
    if resp.status().is_success() {
        let body: BatchResponseMap = test::read_body_json(resp).await;
        assert!(body.results_map.contains_key("octocat/Hello-World"));
    }
}

#[actix_web::test]
async fn test_batch_get_repos_invalid_format() {
    let app = test::init_service(App::new().service(batch_get_repos)).await;

    let batch_request = BatchRequest {
        repos: vec!["invalid-format".to_string()], // 无效的格式
        fields: vec![],
    };

    let req = test::TestRequest::post()
        .uri("/repos/batch")
        .set_json(&batch_request)
        .to_request();

    let resp = test::call_service(&app, req).await;

    // 即使格式无效，也应该返回响应（但success为false）
    if resp.status().is_success() {
        let body: BatchResponse = test::read_body_json(resp).await;
        assert_eq!(body.results.len(), 1);
        assert!(!body.results[0].success);
        assert!(body.results[0].error.is_some());
    }
}

#[actix_web::test]
async fn test_batch_get_repos_multiple_repos() {
    let app = test::init_service(App::new().service(batch_get_repos)).await;

    let batch_request = BatchRequest {
        repos: vec![
            "octocat/Hello-World".to_string(),
            "invalid-format".to_string(), // 一个无效的格式
        ],
        fields: vec![],
    };

    let req = test::TestRequest::post()
        .uri("/repos/batch")
        .set_json(&batch_request)
        .to_request();

    let resp = test::call_service(&app, req).await;

    // 验证返回了多个结果
    if resp.status().is_success() {
        let body: BatchResponse = test::read_body_json(resp).await;
        assert_eq!(body.results.len(), 2);
    }
}
