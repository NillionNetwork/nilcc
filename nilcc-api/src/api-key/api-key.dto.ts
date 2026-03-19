import { z } from "zod";

export const ApiKeyType = z.enum(["account-admin", "user"]);
export type ApiKeyType = z.infer<typeof ApiKeyType>;

export const ApiKey = z
  .object({
    id: z.string().uuid(),
    accountId: z.string().uuid(),
    type: ApiKeyType,
    active: z.boolean(),
    createdAt: z.string().datetime(),
    updatedAt: z.string().datetime(),
  })
  .openapi({ ref: "ApiKey" });
export type ApiKey = z.infer<typeof ApiKey>;

export const CreateApiKeyRequest = z
  .object({
    accountId: z.string().uuid(),
    type: ApiKeyType,
    active: z.boolean().default(true),
  })
  .openapi({ ref: "CreateApiKeyRequest" });
export type CreateApiKeyRequest = z.infer<typeof CreateApiKeyRequest>;

export const UpdateApiKeyRequest = z
  .object({
    id: z.string().uuid(),
    type: ApiKeyType.optional(),
    active: z.boolean().optional(),
  })
  .refine((v) => v.type !== undefined || v.active !== undefined, {
    message: "at least one field must be provided",
  })
  .openapi({ ref: "UpdateApiKeyRequest" });
export type UpdateApiKeyRequest = z.infer<typeof UpdateApiKeyRequest>;

export const DeleteApiKeyRequest = z
  .object({
    id: z.string().uuid(),
  })
  .openapi({ ref: "DeleteApiKeyRequest" });
export type DeleteApiKeyRequest = z.infer<typeof DeleteApiKeyRequest>;

export const ListApiKeysResponse = ApiKey.array().openapi({
  ref: "ListApiKeysResponse",
});
export type ListApiKeysResponse = z.infer<typeof ListApiKeysResponse>;
