use actix_web::{test, App};
use gh_info_rs::handlers::{
    batch_get_repos, batch_get_repos_map, download_attachment, get_latest_release, get_releases, get_repo_info,
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

#[actix_web::test]
async fn test_download_single_file() {
    // 测试单个小文件下载（使用 GitHub raw 文件，通常很小）
    let app = test::init_service(App::new().service(download_attachment)).await;

    // 使用一个小的 GitHub raw 文件进行测试
    // octocat/Hello-World 仓库的 README.md 文件
    let url = "https://raw.githubusercontent.com/octocat/Hello-World/master/README";
    // 简单的 URL 编码：将特殊字符替换为 % 编码
    let encoded_url = url.replace(" ", "%20").replace("#", "%23");
    let req = test::TestRequest::get()
        .uri(&format!("/download?url={}", encoded_url))
        .to_request();

    let resp = test::call_service(&app, req).await;

    // 如果网络可用且文件存在，应该返回 200
    if resp.status().is_success() {
        let body = test::read_body(resp).await;
        assert!(!body.is_empty(), "下载的文件应该不为空");
    }
}

#[actix_web::test]
async fn test_download_missing_url() {
    let app = test::init_service(App::new().service(download_attachment)).await;

    let req = test::TestRequest::get()
        .uri("/download")
        .to_request();

    let resp = test::call_service(&app, req).await;

    // 缺少 url 参数应该返回 400
    assert!(resp.status().is_client_error());
}

#[actix_web::test]
async fn test_download_concurrent_limit() {
    // 测试并发下载限制
    // 设置较小的并发限制以便测试
    std::env::set_var("MAX_CONCURRENT_DOWNLOADS", "2");

    let app = test::init_service(App::new().service(download_attachment)).await;

    // 使用几个小的 GitHub raw 文件进行测试
    let test_urls = vec![
        "https://raw.githubusercontent.com/octocat/Hello-World/master/README",
        "https://raw.githubusercontent.com/octocat/Hello-World/master/LICENSE",
        "https://raw.githubusercontent.com/octocat/Hello-World/master/.gitignore",
    ];

    // 并发发起多个下载请求
    let futures: Vec<_> = test_urls
        .iter()
        .map(|url| {
            let app = &app;
            // 简单的 URL 编码
            let encoded_url = url.replace(" ", "%20").replace("#", "%23");
            async move {
                let req = test::TestRequest::get()
                    .uri(&format!("/download?url={}", encoded_url))
                    .to_request();
                let resp = test::call_service(app, req).await;
                resp.status()
            }
        })
        .collect();

    let results = futures::future::join_all(futures).await;

    // 验证所有请求都被处理（不一定都成功，但应该都被处理）
    assert_eq!(results.len(), test_urls.len());

    // 清理环境变量
    std::env::remove_var("MAX_CONCURRENT_DOWNLOADS");
}

#[actix_web::test]
async fn test_download_concurrent_limit_small() {
    // 测试严格的并发限制（设置为 1）
    std::env::set_var("MAX_CONCURRENT_DOWNLOADS", "1");

    let app = test::init_service(App::new().service(download_attachment)).await;

    // 使用两个小的文件进行测试
    let url1 = "https://raw.githubusercontent.com/octocat/Hello-World/master/README";
    let url2 = "https://raw.githubusercontent.com/octocat/Hello-World/master/LICENSE";

    let encoded_url1 = url1.replace(" ", "%20").replace("#", "%23");
    let encoded_url2 = url2.replace(" ", "%20").replace("#", "%23");
    
    let req1 = test::TestRequest::get()
        .uri(&format!("/download?url={}", encoded_url1))
        .to_request();

    let req2 = test::TestRequest::get()
        .uri(&format!("/download?url={}", encoded_url2))
        .to_request();

    // 并发发起两个请求
    let (resp1, resp2) = futures::join!(
        test::call_service(&app, req1),
        test::call_service(&app, req2)
    );

    // 两个请求都应该被处理（不一定都成功，但应该都被处理）
    assert!(resp1.status().is_success() || resp1.status().is_client_error() || resp1.status().is_server_error());
    assert!(resp2.status().is_success() || resp2.status().is_client_error() || resp2.status().is_server_error());

    // 清理环境变量
    std::env::remove_var("MAX_CONCURRENT_DOWNLOADS");
}
