//! 设备业务服务

use crate::db::RedisPool;
use crate::errors::AppError;
use crate::models::{
    CreateDeviceRequest, CreateDeviceResponse, Device, DeviceConfig, DeviceListQuery,
    PaginatedResponse, Pagination, UpdateDeviceConfigRequest, UpdateDeviceRequest,
};
use crate::repositories::DeviceRepository;
use crate::security::{generate_token, verify_token, TokenType};
use std::sync::Arc;
use uuid::Uuid;

/// 设备业务服务
pub struct DeviceService {
    device_repo: DeviceRepository,
    redis_pool: Arc<RedisPool>,
}

impl DeviceService {
    pub fn new(device_repo: DeviceRepository, redis_pool: Arc<RedisPool>) -> Self {
        Self {
            device_repo,
            redis_pool,
        }
    }

    /// 注册新设备
    pub async fn register(&self, request: CreateDeviceRequest, owner_id: Option<Uuid>) -> Result<CreateDeviceResponse, AppError> {
        // 生成 API Key（使用统一的 token 模块）
        let token_result = generate_token(TokenType::DeviceApiKeyLive)?;

        // 创建设备
        let device = self
            .device_repo
            .create(&request, &token_result.hash, &token_result.display_prefix, owner_id)
            .await?;

        // 获取默认配置
        let config = self
            .device_repo
            .get_config(device.id)
            .await?
            .unwrap_or_default();

        Ok(CreateDeviceResponse {
            device,
            api_key: token_result.token, // 仅此一次返回完整 API Key
            config,
        })
    }

    /// 根据 ID 获取设备
    pub async fn get_by_id(&self, id: Uuid) -> Result<Device, AppError> {
        self.device_repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound("设备不存在".to_string()))
    }

    /// 根据 API Key 验证设备
    pub async fn verify_by_api_key(&self, api_key: &str) -> Result<Device, AppError> {
        // 从 API Key 提取前缀（前 16 个字符）
        if api_key.len() < 16 {
            return Err(AppError::Unauthorized("无效的 API Key".to_string()));
        }

        let prefix = &api_key[..16];

        // 查找设备
        let device = self
            .device_repo
            .find_by_api_key_prefix(prefix)
            .await?
            .ok_or_else(|| AppError::Unauthorized("无效的 API Key".to_string()))?;

        // 验证完整 API Key（使用统一的 token 模块）
        if !verify_token(api_key, &device.api_key_hash)? {
            return Err(AppError::Unauthorized("无效的 API Key".to_string()));
        }

        Ok(device)
    }

    /// 更新设备
    pub async fn update(&self, id: Uuid, request: UpdateDeviceRequest) -> Result<Device, AppError> {
        // 确保设备存在
        self.get_by_id(id).await?;

        // 更新设备
        let device = self.device_repo.update(id, &request).await?;

        // 清除缓存
        self.invalidate_cache(id).await?;

        Ok(device)
    }

    /// 删除设备
    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        // 确保设备存在
        self.get_by_id(id).await?;

        // 删除设备
        self.device_repo.delete(id).await?;

        // 清除缓存
        self.invalidate_cache(id).await?;

        Ok(())
    }

    /// 查询设备列表
    pub async fn list(&self, query: DeviceListQuery) -> Result<PaginatedResponse<Device>, AppError> {
        let (devices, total) = self.device_repo.list(&query).await?;

        let pagination = Pagination::new(query.page, query.page_size, total);

        Ok(PaginatedResponse::new(devices, pagination))
    }

    /// 获取设备配置
    pub async fn get_config(&self, device_id: Uuid) -> Result<DeviceConfig, AppError> {
        // 先检查缓存
        let cache_key = format!("device:config:{}", device_id);
        if let Some(cached) = self.redis_pool.get::<DeviceConfig>(&cache_key).await? {
            return Ok(cached);
        }

        // 确保设备存在
        self.get_by_id(device_id).await?;

        // 获取配置
        let config = self
            .device_repo
            .get_config(device_id)
            .await?
            .ok_or_else(|| AppError::NotFound("设备配置不存在".to_string()))?;

        // 更新缓存
        self.redis_pool.set_ex(&cache_key, &config, 3600).await?;

        Ok(config)
    }

    /// 更新设备配置
    pub async fn update_config(
        &self,
        device_id: Uuid,
        request: UpdateDeviceConfigRequest,
    ) -> Result<DeviceConfig, AppError> {
        // 确保设备存在
        self.get_by_id(device_id).await?;

        // 验证配置逻辑
        if let (Some(low), Some(critical)) = (
            request.low_battery_threshold,
            request.critical_battery_threshold,
        ) {
            if critical >= low {
                return Err(AppError::ValidationError(
                    "临界电量阈值必须小于低电量阈值".to_string(),
                ));
            }
        }

        // 更新配置
        let config = self.device_repo.update_config(device_id, &request).await?;

        // 清除缓存
        self.invalidate_cache(device_id).await?;

        Ok(config)
    }

    /// 轮换 API Key
    pub async fn rotate_api_key(&self, device_id: Uuid) -> Result<String, AppError> {
        // 确保设备存在
        self.get_by_id(device_id).await?;

        // 生成新的 API Key（使用统一的 token 模块）
        let token_result = generate_token(TokenType::DeviceApiKeyLive)?;

        // 更新数据库
        self.device_repo
            .rotate_api_key(device_id, &token_result.hash, &token_result.display_prefix)
            .await?;

        // 清除缓存
        self.invalidate_cache(device_id).await?;

        Ok(token_result.token)
    }

    /// 清除设备相关缓存
    async fn invalidate_cache(&self, device_id: Uuid) -> Result<(), AppError> {
        let keys = vec![
            format!("device:config:{}", device_id),
            format!("battery:latest:{}", device_id),
        ];

        for key in keys {
            self.redis_pool.del(&key).await?;
        }

        Ok(())
    }
}
