use actix_web::{App, HttpServer};
use gh_info_rs::cache::get_cache_manager;
use gh_info_rs::rate_limit::get_rate_limit_manager;
use gh_info_rs::handlers::{
    batch_get_repos, batch_get_repos_map, download_attachment, get_latest_release, get_latest_release_pre, get_releases, get_repo_info,
    health, health_check,
};
use gh_info_rs::ApiDoc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // 初始化日志记录器
    // 可以通过环境变量 LOG_LEVEL 设置日志级别，例如：LOG_LEVEL=debug 或 LOG_LEVEL=info
    // 如果未设置 LOG_LEVEL，则尝试从 RUST_LOG 读取（向后兼容）
    let log_level = std::env::var("LOG_LEVEL")
        .or_else(|_| std::env::var("RUST_LOG"))
        .unwrap_or_else(|_| "info".to_string());

    // 创建自定义环境变量配置，优先使用 LOG_LEVEL，如果没有则使用 RUST_LOG
    let env = env_logger::Env::default()
        .filter_or("RUST_LOG", &log_level);
    env_logger::Builder::from_env(env).init();

    // 从环境变量获取绑定地址，默认为 0.0.0.0:8080（Docker 友好）
    let bind_addr = std::env::var("BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

    println!("🚀 GitHub API 信息收集服务启动中...");
    println!("📡 服务地址: http://{}", bind_addr);
    println!("📚 可用端点:");
    println!("   GET  /                                    - 健康检查和基本信息");
    println!("   GET  /health                              - 健康检查端点");
    println!("   GET  /repos/{{owner}}/{{repo}}              - 获取仓库基本信息");
    println!("   GET  /repos/{{owner}}/{{repo}}/releases     - 获取所有 releases");
    println!("   GET  /repos/{{owner}}/{{repo}}/releases/latest - 获取最新 release");
    println!("   GET  /repos/{{owner}}/{{repo}}/releases/latest/pre - 获取最新 release（包括 pre-release）");
    println!("   POST /repos/batch                          - 批量获取多个仓库信息（数组格式）");
    println!("   POST /repos/batch/map                      - 批量获取多个仓库信息（Map 格式）");
    println!("   GET  /download?url={{url}}                 - 下载附件文件（支持缓存）");
    println!("   GET  /swagger-ui/*                         - API 文档页面");
    println!();

    // 初始化缓存管理器（加载持久化缓存）
    log::info!("正在初始化缓存管理器...");
    get_cache_manager().await;
    log::info!("缓存管理器初始化完成");

    // 初始化限流管理器
    log::info!("正在初始化限流管理器...");
    get_rate_limit_manager().await;
    log::info!("限流管理器初始化完成");

    HttpServer::new(|| {
        App::new()
            .service(
                SwaggerUi::new("/swagger-ui/{_:.*}")
                    .url("/api-doc/openapi.json", ApiDoc::openapi()),
            )
            .service(health_check)
            .service(health)
            .service(get_repo_info)
            .service(get_releases)
            .service(get_latest_release)
            .service(get_latest_release_pre)
            .service(batch_get_repos)
            .service(batch_get_repos_map)
            .service(download_attachment)
    })
    .bind(&bind_addr)?
    .run()
    .await
}
