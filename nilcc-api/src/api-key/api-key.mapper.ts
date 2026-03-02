import type { ApiKey } from "./api-key.dto";
import type { ApiKeyEntity } from "./api-key.entity";

export const apiKeyMapper = {
  entityToResponse(entity: ApiKeyEntity): ApiKey {
    return {
      id: entity.id,
      accountId: entity.accountId,
      type: entity.type,
      active: entity.active,
      createdAt: entity.createdAt.toISOString(),
      updatedAt: entity.updatedAt.toISOString(),
    };
  },
};
