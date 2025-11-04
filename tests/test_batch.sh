#!/bin/bash

# 批量查询测试脚本
# 用于直观测试 /repos/batch 和 /repos/batch/map 接口

# 默认服务器地址
SERVER_URL="${SERVER_URL:-http://localhost:8080}"

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# 打印分隔线
print_separator() {
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
}

# 打印标题
print_title() {
    echo -e "\n${BLUE}► $1${NC}"
}

# 打印简短的请求信息
print_request_short() {
    if command -v jq &> /dev/null; then
        echo "$1" | jq '{repos: .repos, fields: .fields}'
    else
        echo "$1"
    fi
}

# 打印 JSON（使用 jq 美化，如果没有则原样输出）
print_json() {
    if command -v jq &> /dev/null; then
        echo "$1" | jq .
    else
        echo "$1"
    fi
}

# 打印精简的 JSON 结构（只显示关键字段和类型）
print_json_structure() {
    if command -v jq &> /dev/null; then
        # 隐藏长文本字段，截断长字符串
        echo "$1" | jq 'def truncate(s): if type == "string" and (s | length) > 80 then (s | .[0:80] + "...") else s end;
        walk(if type == "object" then
            with_entries(select(.key != "description" and .key != "body" and .key != "message" and .key != "commit_message"))
            else . end) |
        walk(if type == "string" then truncate(.) else . end)'
    else
        echo "$1"
    fi
}

# 测试 batch 端点（数组格式）
test_batch_array() {
    print_title "测试 /repos/batch (数组格式)"

    REQUEST='{
        "repos": [
            "rust-lang/rust",
            "microsoft/vscode",
            "facebook/react"
        ],
        "fields": ["repo_info", "latest_release"]
    }'

    echo -e "${YELLOW}请求:${NC} "
    print_request_short "$REQUEST"

    RESPONSE=$(curl -s -X POST "${SERVER_URL}/repos/batch" \
        -H "Content-Type: application/json" \
        -d "$REQUEST" 2>/dev/null)

    if [ $? -eq 0 ] && [ -n "$RESPONSE" ]; then
        echo -e "\n${YELLOW}响应结构:${NC}"
        print_json_structure "$RESPONSE"

        # 统计
        if command -v jq &> /dev/null; then
            SUCCESS_COUNT=$(echo "$RESPONSE" | jq '[.results[] | select(.success == true)] | length')
            TOTAL_COUNT=$(echo "$RESPONSE" | jq '.results | length')
            echo -e "\n${GREEN}✓ 成功: ${SUCCESS_COUNT}/${TOTAL_COUNT}${NC}"
        fi
    else
        echo -e "${RED}✗ 请求失败${NC}"
    fi
}

# 测试 batch/map 端点（Map 格式）
test_batch_map() {
    print_title "测试 /repos/batch/map (Map 格式)"

    REQUEST='{
        "repos": [
            "rust-lang/rust",
            "microsoft/vscode",
            "facebook/react"
        ],
        "fields": ["repo_info", "latest_release"]
    }'

    echo -e "${YELLOW}请求:${NC} "
    print_request_short "$REQUEST"

    RESPONSE=$(curl -s -X POST "${SERVER_URL}/repos/batch/map" \
        -H "Content-Type: application/json" \
        -d "$REQUEST" 2>/dev/null)

    if [ $? -eq 0 ] && [ -n "$RESPONSE" ]; then
        echo -e "\n${YELLOW}响应结构:${NC}"
        print_json_structure "$RESPONSE"

        # 统计
        if command -v jq &> /dev/null; then
            SUCCESS_COUNT=$(echo "$RESPONSE" | jq '[.results_map[] | select(.success == true)] | length')
            TOTAL_COUNT=$(echo "$RESPONSE" | jq '.results_map | length')
            echo -e "\n${GREEN}✓ 成功: ${SUCCESS_COUNT}/${TOTAL_COUNT}${NC}"
        fi
    else
        echo -e "${RED}✗ 请求失败${NC}"
    fi
}

# 测试获取所有字段（默认）
test_batch_all_fields() {
    print_title "测试 /repos/batch (获取所有字段，不指定 fields)"

    REQUEST='{
        "repos": [
            "octocat/Hello-World"
        ]
    }'

    echo -e "${YELLOW}请求:${NC} "
    print_request_short "$REQUEST"

    RESPONSE=$(curl -s -X POST "${SERVER_URL}/repos/batch" \
        -H "Content-Type: application/json" \
        -d "$REQUEST" 2>/dev/null)

    if [ $? -eq 0 ] && [ -n "$RESPONSE" ]; then
        echo -e "\n${YELLOW}响应结构:${NC}"
        if command -v jq &> /dev/null; then
            # 显示字段结构，但隐藏长文本
            echo "$RESPONSE" | jq '{
                results: [.results[] | {
                    repo: .repo,
                    success: .success,
                    fields: (. | keys - ["repo", "success", "error"])
                }]
            }'
        else
            print_json_structure "$RESPONSE"
        fi
    else
        echo -e "${RED}✗ 请求失败${NC}"
    fi
}

# 测试错误情况（无效的仓库格式）
test_batch_invalid_format() {
    print_title "测试 /repos/batch (包含无效格式的仓库)"

    REQUEST='{
        "repos": [
            "rust-lang/rust",
            "invalid-format",
            "microsoft/vscode"
        ]
    }'

    echo -e "${YELLOW}请求:${NC} "
    print_request_short "$REQUEST"

    RESPONSE=$(curl -s -X POST "${SERVER_URL}/repos/batch" \
        -H "Content-Type: application/json" \
        -d "$REQUEST" 2>/dev/null)

    if [ $? -eq 0 ] && [ -n "$RESPONSE" ]; then
        echo -e "\n${YELLOW}响应结构:${NC}"
        if command -v jq &> /dev/null; then
            echo "$RESPONSE" | jq '{
                results: [.results[] | {
                    repo: .repo,
                    success: .success,
                    error: .error
                }]
            }'
        else
            print_json_structure "$RESPONSE"
        fi
    else
        echo -e "${RED}✗ 请求失败${NC}"
    fi
}

# 测试错误情况（空列表）
test_batch_empty_list() {
    print_title "测试 /repos/batch (空仓库列表)"

    REQUEST='{
        "repos": []
    }'

    echo -e "${YELLOW}请求:${NC} "
    print_request_short "$REQUEST"

    HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" -X POST "${SERVER_URL}/repos/batch" \
        -H "Content-Type: application/json" \
        -d "$REQUEST" 2>/dev/null)

    RESPONSE=$(curl -s -X POST "${SERVER_URL}/repos/batch" \
        -H "Content-Type: application/json" \
        -d "$REQUEST" 2>/dev/null)

    echo -e "\n${YELLOW}HTTP 状态码: ${HTTP_CODE}${NC}"

    if [ "$HTTP_CODE" != "200" ] && [ -n "$RESPONSE" ]; then
        echo -e "${YELLOW}错误响应:${NC}"
        print_json_structure "$RESPONSE"
    elif [ "$HTTP_CODE" == "200" ] && [ -n "$RESPONSE" ]; then
        echo -e "${YELLOW}响应结构:${NC}"
        print_json_structure "$RESPONSE"
    fi
}

# 主函数
main() {
    echo -e "${CYAN}GitHub API 批量查询测试${NC}"
    echo -e "${YELLOW}服务器: ${SERVER_URL}${NC}\n"

    # 检查服务器是否可用
    if ! curl -s "${SERVER_URL}/repos/octocat/Hello-World" > /dev/null 2>&1; then
        echo -e "${RED}✗ 无法连接到服务器 ${SERVER_URL}${NC}"
        echo -e "${YELLOW}提示: SERVER_URL=http://localhost:8080 ./test_batch.sh${NC}"
        exit 1
    fi

    # 运行所有测试
    test_batch_array
    test_batch_map
    test_batch_all_fields
    test_batch_invalid_format
    test_batch_empty_list

    echo -e "\n${GREEN}✓ 所有测试完成${NC}"
}

# 运行主函数
main

