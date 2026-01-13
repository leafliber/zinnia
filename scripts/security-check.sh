#!/usr/bin/env bash
# ===========================================
# Zinnia 部署安全配置检查清单
# ===========================================
# 运行此脚本验证所有安全配置是否符合最小权限原则

set -euo pipefail

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_pass() { echo -e "${GREEN}[✓]${NC} $*"; }
log_fail() { echo -e "${RED}[✗]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[⚠]${NC} $*"; }
log_info() { echo -e "${BLUE}[ℹ]${NC} $*"; }

echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  Zinnia 安全配置检查${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

ERRORS=0
WARNINGS=0

# 1. 检查 secrets 文件权限
echo "1. Secrets 文件权限检查"
echo "---"
for secret in db_password redis_password jwt_secret encryption_key smtp_password recaptcha_secret; do
    if [ -f "secrets/$secret" ]; then
        perms=$(stat -f "%Lp" "secrets/$secret" 2>/dev/null || stat -c "%a" "secrets/$secret" 2>/dev/null)
        if [ "$perms" = "600" ] || [ "$perms" = "400" ]; then
            log_pass "secrets/$secret: $perms (安全)"
        else
            log_warn "secrets/$secret: $perms (建议 600 或 400)"
            ((WARNINGS++))
        fi
    else
        log_fail "secrets/$secret: 不存在"
        ((ERRORS++))
    fi
done
echo ""

# 2. 检查 docker-compose.yml 中的 secrets mode
echo "2. Docker Secrets Mode 配置检查"
echo "---"
if grep -q "mode: 0400" docker-compose.prod.yml; then
    log_pass "Docker secrets mode 设置为 0400 (仅 root 可读)"
elif grep -q "mode: 0440" docker-compose.prod.yml; then
    log_pass "Docker secrets mode 设置为 0440 (root 和 group 可读)"
elif grep -q "mode: 0444" docker-compose.prod.yml; then
    log_fail "Docker secrets mode 设置为 0444 (所有用户可读 - 不安全)"
    ((ERRORS++))
else
    log_warn "未显式设置 Docker secrets mode (将使用默认 0444)"
    ((WARNINGS++))
fi
echo ""

# 3. 检查 Dockerfile 安全配置
echo "3. Dockerfile 安全配置检查"
echo "---"
if grep -q "gosu" Dockerfile; then
    log_pass "使用 gosu 实现权限降级"
else
    log_fail "未安装 gosu - 无法安全降权"
    ((ERRORS++))
fi

if grep -q "USER.*zinnia" Dockerfile; then
    log_warn "Dockerfile 中直接切换到非 root 用户 (建议在 entrypoint 中降权)"
    ((WARNINGS++))
fi

if grep -q "tini" Dockerfile; then
    log_pass "使用 tini 作为 init 进程 (正确处理信号)"
else
    log_warn "未使用 tini (建议安装)"
    ((WARNINGS++))
fi
echo ""

# 4. 检查 entrypoint.sh
echo "4. Entrypoint 脚本安全检查"
echo "---"
if grep -q "set -euo pipefail" scripts/entrypoint.sh; then
    log_pass "启用严格错误处理 (set -euo pipefail)"
else
    log_warn "未启用严格错误处理"
    ((WARNINGS++))
fi

if grep -q "gosu.*zinnia" scripts/entrypoint.sh; then
    log_pass "使用 gosu 降权到 zinnia 用户"
else
    log_fail "entrypoint 未使用 gosu 降权"
    ((ERRORS++))
fi
echo ""

# 5. 检查容器安全选项
echo "5. 容器安全选项检查"
echo "---"
if grep -q "no-new-privileges:true" docker-compose.prod.yml; then
    log_pass "启用 no-new-privileges (防止权限提升)"
else
    log_warn "未启用 no-new-privileges"
    ((WARNINGS++))
fi

if grep -q "read_only: true" docker-compose.prod.yml; then
    log_pass "启用只读根文件系统"
elif grep -q "read_only: false" docker-compose.prod.yml; then
    log_warn "根文件系统可写 (考虑启用 read_only + tmpfs)"
    ((WARNINGS++))
fi

if grep -q "tmpfs:" docker-compose.prod.yml; then
    log_pass "配置了 tmpfs 临时目录"
else
    log_warn "未配置 tmpfs (建议为 /tmp 配置)"
    ((WARNINGS++))
fi
echo ""

# 6. 检查网络隔离
echo "6. 网络隔离检查"
echo "---"
if grep -q "internal: true" docker-compose.prod.yml; then
    log_pass "后端网络配置为 internal (隔离外部访问)"
else
    log_warn "后端网络未设置为 internal"
    ((WARNINGS++))
fi
echo ""

# 7. 检查资源限制
echo "7. 资源限制检查"
echo "---"
if grep -A 5 "deploy:" docker-compose.prod.yml | grep -q "limits:"; then
    log_pass "配置了资源限制 (防止资源耗尽攻击)"
else
    log_warn "未配置资源限制"
    ((WARNINGS++))
fi
echo ""

# 8. 检查日志配置
echo "8. 日志配置检查"
echo "---"
if grep -q "max-size:" docker-compose.prod.yml; then
    log_pass "配置了日志轮转 (防止磁盘占满)"
else
    log_warn "未配置日志轮转"
    ((WARNINGS++))
fi
echo ""

# 9. 检查环境变量中的敏感信息
echo "9. 环境变量安全检查"
echo "---"
if grep -q "__DB_PASS__\|__REDIS_PASS__" docker-compose.prod.yml; then
    log_pass "使用占位符传递敏感信息 (由 entrypoint 替换)"
else
    log_warn "环境变量中可能直接包含密码"
    ((WARNINGS++))
fi
echo ""

# 10. 检查健康检查配置
echo "10. 健康检查配置"
echo "---"
if grep -q "start_period: 60s" docker-compose.prod.yml; then
    log_pass "健康检查配置了合理的启动时间 (60s)"
else
    log_warn "健康检查启动时间可能不足"
    ((WARNINGS++))
fi
echo ""

# 总结
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  检查结果总结${NC}"
echo -e "${BLUE}========================================${NC}"
if [ $ERRORS -eq 0 ] && [ $WARNINGS -eq 0 ]; then
    echo -e "${GREEN}✓ 所有安全检查通过！配置符合最小权限原则。${NC}"
    exit 0
elif [ $ERRORS -eq 0 ]; then
    echo -e "${YELLOW}⚠ 有 $WARNINGS 个警告，建议改进。${NC}"
    exit 0
else
    echo -e "${RED}✗ 发现 $ERRORS 个错误和 $WARNINGS 个警告，请修复后再部署。${NC}"
    exit 1
fi
