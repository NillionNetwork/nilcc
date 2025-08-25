import {
  type EntityMetadata,
  type EntitySubscriberInterface,
  EventSubscriber,
  type InsertEvent,
  type LoadEvent,
} from "typeorm";
import "reflect-metadata";
import type { Context, Next } from "hono";
import { InitialState1754946297570 } from "migrations/1754946297570-InitialState";
import { Account1755033746208 } from "migrations/1755033746208-Account";
import { WorkloadAccount1755195024670 } from "migrations/1755195024670-WorkloadAccount";
import { WorkloadDomain1755621623214 } from "migrations/1755621623214-WorkloadDomain";
import { DockerCredentials1755638110882 } from "migrations/1755638110882-DockerCredentials";
import { Tiers1756136984989 } from "migrations/1756136984989-Tiers";
import { AccountCredits1756146720184 } from "migrations/1756146720184-AccountCredits";
import { DataSource } from "typeorm";
import type { EnvVars } from "#/env";
import { MetalInstanceEntity } from "#/metal-instance/metal-instance.entity";
import {
  WorkloadEntity,
  WorkloadEventEntity,
} from "#/workload/workload.entity";
import { AccountEntity } from "./account/account.entity";
import { WorkloadTierEntity } from "./workload-tier/workload-tier.entity";

export async function buildDataSource(config: EnvVars): Promise<DataSource> {
  const dataSource = new DataSource({
    type: "postgres",
    url: config.dbUri,
    entities: [
      AccountEntity,
      WorkloadEntity,
      MetalInstanceEntity,
      WorkloadEventEntity,
      WorkloadTierEntity,
    ],
    subscribers: [NullToUndefinedSubscriber],
    // We can't use globs (e.g. `migrations/*.ts`) here because of some very reasonable problem with typescript
    migrations: [
      InitialState1754946297570,
      Account1755033746208,
      WorkloadAccount1755195024670,
      WorkloadDomain1755621623214,
      DockerCredentials1755638110882,
      Tiers1756136984989,
      AccountCredits1756146720184,
    ],
    synchronize: false,
    logging: false,
    migrationsRun: true,
  });

  return dataSource;
}

export function transactionMiddleware(dataSource: DataSource) {
  return async (c: Context, next: Next) => {
    const queryRunner = dataSource.createQueryRunner();

    try {
      await queryRunner.connect();
      await queryRunner.startTransaction();

      c.set("txQueryRunner", queryRunner);

      await next();
      const statusCode = c.res.status;
      if (statusCode >= 200 && statusCode < 300) {
        await queryRunner.commitTransaction();
      } else {
        await queryRunner.rollbackTransaction();
      }
    } catch (error) {
      await queryRunner.rollbackTransaction();
      throw error;
    } finally {
      await queryRunner.release();
    }
  };
}

// Map nulls to undefined, see https://github.com/typeorm/typeorm/issues/2934
@EventSubscriber()
export class NullToUndefinedSubscriber implements EntitySubscriberInterface {
  afterLoad?(_entity: object, event?: LoadEvent<object>) {
    if (!event) {
      return;
    }
    this.handleEvent(event);
  }

  afterInsert(event: InsertEvent<object>) {
    this.handleEvent(event);
  }

  handleEvent(event: { entity: object; metadata: EntityMetadata }) {
    const eventEntity = event.entity;
    for (const col of event.metadata.columns) {
      if (!col.isNullable) {
        continue;
      }
      const val = Reflect.get(eventEntity, col.propertyName);
      if (val === null) {
        Reflect.set(eventEntity, col.propertyName, undefined);
      }
    }
  }
}
