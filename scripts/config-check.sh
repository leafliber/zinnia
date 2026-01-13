#!/usr/bin/env bash
# ===========================================
# 配置一致性验证脚本
# ===========================================
# 检查配置加载是否正确

set -euo pipefail

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[检查]${NC} $*"; }
log_pass() { echo -e "${GREEN}[✓]${NC} $*"; }
log_fail() { echo -e "${RED}[✗]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[⚠]${NC} $*"; }

echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  配置一致性验证${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

ERRORS=0

# 1. 检查环境变量一致性
log_info "检查环境变量命名..."
if grep -q "APP_ENV: production" docker-compose.prod.yml; then
    log_pass "docker-compose.prod.yml 包含 APP_ENV"
else
    log_fail "docker-compose.prod.yml 缺少 APP_ENV"
    ((ERRORS++))
fi

if grep -q "RUN_MODE: production" docker-compose.prod.yml; then
    log_warn "docker-compose.prod.yml 仍包含 RUN_MODE（冗余但无害）"
fi

# 2. 检查代码中的环境变量读取
log_info "检查代码中的环境变量使用..."
if grep -q 'env::var("APP_ENV")' src/config/settings.rs; then
    log_pass "代码使用 APP_ENV 变量"
else
    log_fail "代码未使用 APP_ENV"
    ((ERRORS++))
fi

# 3. 检查配置文件（已废弃，使用纯环境变量）
log_info "检查配置文件..."
if [ -d "config" ]; then
    log_warn "config/ 文件夹仍然存在（建议已移除，使用纯环境变量）"
else
    log_pass "config/ 文件夹已移除（推荐：纯环境变量配置）"
fi

# 4. 检查 Dockerfile 是否复制配置
log_info "检查 Dockerfile 配置复制..."
if grep -q "COPY.*config" Dockerfile; then
    log_warn "Dockerfile 复制 config/ 到镜像（不符合容器化最佳实践）"
    log_info "  建议: 完全通过环境变量管理配置"
else
    log_pass "Dockerfile 未复制 config/（推荐）"
fi

# 5. 检查环境变量完整性
log_info "检查必需的环境变量和 Secrets..."
required_env_vars=(
    "DATABASE_URL"
    "REDIS_URL"
)

required_secrets=(
    "jwt_secret"
    "encryption_key"
    "db_password"
    "redis_password"
)

for var in "${required_env_vars[@]}"; do
    if grep -q "$var" docker-compose.prod.yml; then
        log_pass "$var 在 docker-compose.prod.yml 中配置"
    else
        log_fail "$var 未在 docker-compose.prod.yml 中配置"
        ((ERRORS++))
    fi
done

for secret in "${required_secrets[@]}"; do
    if grep -q "secrets:" docker-compose.prod.yml && grep -A 10 "secrets:" docker-compose.prod.yml | grep -q "$secret"; then
        log_pass "Secret $secret 已在服务中挂载"
    else
        log_warn "Secret $secret 可能未在服务中挂载"
    fi
    
    if [ -f "secrets/$secret" ]; then
        log_pass "secrets/$secret 文件存在"
    else
        log_fail "secrets/$secret 文件不存在"
        ((ERRORS++))
    fi
done

# 6. 检查配置优先级文档
log_info "检查配置文档..."
if [ -f ".env.example" ]; then
    log_pass ".env.example 存在"
else
    log_warn ".env.example 不存在（建议创建）"
fi

if [ -f "STRUCTURE_ANALYSIS.md" ]; then
    log_pass "STRUCTURE_ANALYSIS.md 存在（配置分析文档）"
fi

# 总结
echo ""
echo -e "${BLUE}========================================${NC}"
if [ $ERRORS -eq 0 ]; then
    echo -e "${GREEN}✓ 配置检查通过！${NC}"
    echo ""
    echo "配置加载顺序："
    echo "  1. 代码默认值（根据 APP_ENV 自适应）"
    echo "  2. ZINNIA_* 环境变量（覆盖默认值）"
    echo "  3. DATABASE_URL, REDIS_URL 等（直接环境变量）"
    echo ""
    echo "生产部署时，应用会："
    echo "  1. 读取 APP_ENV=production"
    echo "  2. 应用生产环境默认值"
    echo "  3. 用环境变量覆盖配置"
    exit 0
else
    echo -e "${RED}✗ 发现 $ERRORS 个问题${NC}"
    echo ""
    echo "建议采取以下措施："
    echo "  1. 确保 docker-compose.prod.yml 设置 APP_ENV=production"
    echo "  2. 确保所有必需的环境变量已配置（参考 .env.template）"
    echo "  3. 检查所有 secrets 文件是否存在且权限正确"
    exit 1
fi
