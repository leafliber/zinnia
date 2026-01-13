#!/usr/bin/env bash
# ===========================================
# Zinnia 部署前预检查脚本
# ===========================================
# 功能：检查部署所需的所有前置条件
# 使用：./scripts/preflight-check.sh
# ===========================================

set -euo pipefail

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# 项目根目录
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

# 计数器
ERRORS=0
WARNINGS=0

# ===========================================
# 工具函数
# ===========================================

log_info() { echo -e "${BLUE}[INFO]${NC} $*"; }
log_success() { echo -e "${GREEN}[✓]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[⚠]${NC} $*"; ((WARNINGS++)); }
log_error() { echo -e "${RED}[✗]${NC} $*"; ((ERRORS++)); }

print_header() {
    echo ""
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}  $*${NC}"
    echo -e "${BLUE}========================================${NC}"
}

# ===========================================
# 检查函数
# ===========================================

check_docker() {
    print_header "Docker 环境检查"
    
    # Docker
    if command -v docker >/dev/null 2>&1; then
        local version
        version=$(docker --version 2>/dev/null | cut -d' ' -f3 | tr -d ',')
        log_success "Docker 已安装 (版本: $version)"
    else
        log_error "Docker 未安装"
        return 1
    fi
    
    # Docker Compose
    if docker compose version >/dev/null 2>&1; then
        local compose_version
        compose_version=$(docker compose version --short 2>/dev/null)
        log_success "Docker Compose 已安装 (版本: $compose_version)"
    elif command -v docker-compose >/dev/null 2>&1; then
        log_warn "使用旧版 docker-compose，建议升级到 Docker Compose V2"
    else
        log_error "Docker Compose 未安装"
    fi
    
    # Docker 运行状态
    if docker info >/dev/null 2>&1; then
        log_success "Docker 守护进程正在运行"
    else
        log_error "Docker 守护进程未运行，请启动 Docker"
    fi
}

check_files() {
    print_header "必要文件检查"
    
    local required_files=(
        "Dockerfile"
        "docker-compose.prod.yml"
        "scripts/entrypoint.sh"
        "scripts/redis-entrypoint.sh"
        "nginx/nginx.conf"
        "nginx/conf.d/zinnia.conf"
        "Cargo.toml"
        "Cargo.lock"
    )
    
    for file in "${required_files[@]}"; do
        if [ -f "$file" ]; then
            log_success "$file 存在"
        else
            log_error "$file 不存在"
        fi
    done
    
    # 检查脚本可执行权限
    local scripts=(
        "scripts/entrypoint.sh"
        "scripts/redis-entrypoint.sh"
        "scripts/deploy.sh"
    )
    
    for script in "${scripts[@]}"; do
        if [ -f "$script" ] && [ ! -x "$script" ]; then
            log_warn "$script 没有执行权限，正在修复..."
            chmod +x "$script"
            log_success "$script 权限已修复"
        fi
    done
}

check_secrets() {
    print_header "Secrets 文件检查"
    
    local secrets_dir="./secrets"
    
    if [ ! -d "$secrets_dir" ]; then
        log_error "secrets 目录不存在"
        log_info "请运行: mkdir -p secrets && chmod 700 secrets"
        return 1
    fi
    
    # 检查目录权限
    local dir_perms
    dir_perms=$(stat -f "%Lp" "$secrets_dir" 2>/dev/null || stat -c "%a" "$secrets_dir" 2>/dev/null)
    if [ "$dir_perms" = "700" ]; then
        log_success "secrets 目录权限正确 (700)"
    else
        log_warn "secrets 目录权限建议设为 700 (当前: $dir_perms)"
    fi
    
    # 必需的 secrets
    local required_secrets=(
        "db_password:数据库密码"
        "redis_password:Redis 密码"
        "jwt_secret:JWT 密钥"
        "encryption_key:加密密钥"
    )
    
    # 可选的 secrets
    local optional_secrets=(
        "smtp_password:SMTP 邮箱密码"
        "recaptcha_secret:reCAPTCHA 密钥"
    )
    
    for secret in "${required_secrets[@]}"; do
        local file="${secret%%:*}"
        local desc="${secret#*:}"
        local path="$secrets_dir/$file"
        
        if [ -f "$path" ]; then
            local size
            size=$(wc -c < "$path" | tr -d ' ')
            if [ "$size" -gt 1 ]; then
                log_success "$file 存在 ($desc)"
            else
                log_error "$file 为空 ($desc) - 必须设置有效值"
            fi
        else
            log_error "$file 不存在 ($desc)"
        fi
    done
    
    for secret in "${optional_secrets[@]}"; do
        local file="${secret%%:*}"
        local desc="${secret#*:}"
        local path="$secrets_dir/$file"
        
        if [ -f "$path" ]; then
            local size
            size=$(wc -c < "$path" | tr -d ' ')
            if [ "$size" -gt 1 ]; then
                log_success "$file 存在 ($desc)"
            else
                log_info "$file 为空 ($desc) - 相关功能将被禁用"
            fi
        else
            log_info "$file 不存在 ($desc) - 相关功能将被禁用"
            # 创建空文件以避免 Docker secrets 错误
            touch "$path"
            log_info "已创建空的 $file"
        fi
    done
}

check_env() {
    print_header "环境配置检查"
    
    local env_file=".env.production"
    
    if [ -f "$env_file" ]; then
        log_success "$env_file 存在"
        
        # 检查关键配置
        if grep -q "^POSTGRES_USER=" "$env_file"; then
            log_success "POSTGRES_USER 已配置"
        else
            log_warn "POSTGRES_USER 未配置，将使用默认值 'zinnia'"
        fi
        
        if grep -q "^DOMAIN=" "$env_file"; then
            local domain
            domain=$(grep "^DOMAIN=" "$env_file" | cut -d'=' -f2)
            if [ "$domain" = "localhost" ]; then
                log_info "域名配置为 localhost，将使用 HTTP 模式"
            else
                log_success "域名配置为: $domain"
            fi
        else
            log_info "DOMAIN 未配置，将使用 localhost"
        fi
    else
        log_warn "$env_file 不存在，请运行 ./scripts/deploy.sh 进行配置"
    fi
}

check_ports() {
    print_header "端口检查"
    
    local ports=("80" "443" "5432" "6379")
    
    for port in "${ports[@]}"; do
        if lsof -Pi ":$port" -sTCP:LISTEN -t >/dev/null 2>&1; then
            local process
            process=$(lsof -Pi ":$port" -sTCP:LISTEN 2>/dev/null | tail -1 | awk '{print $1}')
            log_warn "端口 $port 已被占用 (进程: $process)"
        else
            log_success "端口 $port 可用"
        fi
    done
}

check_disk_space() {
    print_header "磁盘空间检查"
    
    # 获取可用空间（单位：GB）
    local available
    available=$(df -g . 2>/dev/null | tail -1 | awk '{print $4}' || df -BG . 2>/dev/null | tail -1 | awk '{print $4}' | tr -d 'G')
    
    if [ -n "$available" ]; then
        if [ "$available" -ge 10 ]; then
            log_success "可用磁盘空间: ${available}GB (建议至少 10GB)"
        elif [ "$available" -ge 5 ]; then
            log_warn "可用磁盘空间: ${available}GB (建议至少 10GB)"
        else
            log_error "可用磁盘空间不足: ${available}GB (需要至少 5GB)"
        fi
    else
        log_info "无法检测磁盘空间"
    fi
}

check_network() {
    print_header "网络检查"
    
    # 检查能否访问 Docker Hub
    if curl -sf --max-time 5 https://registry-1.docker.io/v2/ >/dev/null 2>&1; then
        log_success "可以访问 Docker Hub"
    else
        log_warn "无法访问 Docker Hub，可能影响镜像拉取"
    fi
    
    # 检查能否访问 crates.io (Rust 包)
    if curl -sf --max-time 5 https://crates.io >/dev/null 2>&1; then
        log_success "可以访问 crates.io"
    else
        log_warn "无法访问 crates.io，可能影响 Rust 依赖构建"
    fi
}

# ===========================================
# 主流程
# ===========================================

main() {
    echo ""
    echo -e "${GREEN}"
    cat << "EOF"
 ______     ______     __   __     __   __     __     ______    
/\___  \   /\  ___\   /\ "-.\ \   /\ "-.\ \   /\ \   /\  __ \   
\/_/  /__  \ \  __\   \ \ \-.  \  \ \ \-.  \  \ \ \  \ \  __ \  
  /\_____\  \ \_____\  \ \_\\"\_\  \ \_\\"\_\  \ \_\  \ \_\ \_\ 
  \/_____/   \/_____/   \/_/ \/_/   \/_/ \/_/   \/_/   \/_/\/_/ 
                                                                 
EOF
    echo -e "${NC}"
    echo "部署前预检查 v1.0"
    echo ""
    
    check_docker
    check_files
    check_secrets
    check_env
    check_ports
    check_disk_space
    check_network
    
    # 总结
    print_header "检查结果"
    
    if [ $ERRORS -eq 0 ] && [ $WARNINGS -eq 0 ]; then
        echo -e "${GREEN}✓ 所有检查通过！可以开始部署。${NC}"
        echo ""
        echo "运行以下命令开始部署："
        echo "  ./scripts/deploy.sh"
        exit 0
    elif [ $ERRORS -eq 0 ]; then
        echo -e "${YELLOW}⚠ 检查完成，有 $WARNINGS 个警告。${NC}"
        echo "建议处理警告后再部署，或确认可以接受这些问题。"
        exit 0
    else
        echo -e "${RED}✗ 检查失败，有 $ERRORS 个错误，$WARNINGS 个警告。${NC}"
        echo "请修复上述错误后再尝试部署。"
        exit 1
    fi
}

# 运行
main "$@"
