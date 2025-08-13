import type { Account } from "./account.dto";
import type { AccountEntity } from "./account.entity";

export const accountMapper = {
  entityToResponse(account: AccountEntity): Account {
    return {
      id: account.id,
      createdAt: account.createdAt.toISOString(),
      name: account.name,
      apiToken: account.apiToken,
    };
  },
};
