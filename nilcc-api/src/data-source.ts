import "reflect-metadata";
import type { Context, Next } from "hono";
import type { Logger } from "pino";
import { DataSource } from "typeorm";
import { type EnvVars, FeatureFlag, hasFeatureFlag } from "#/env";
import { MetalInstanceEntity } from "#/metal-instance/metal-instance.entity";
import {
  WorkloadEntity,
  WorkloadEventEntity,
} from "#/workload/workload.entity";

export async function buildDataSource(
  config: EnvVars,
  log: Logger,
): Promise<DataSource> {
  const synchronize = hasFeatureFlag(
    config.enabledFeatures,
    FeatureFlag.MIGRATIONS,
  );

  const dataSource = new DataSource({
    type: "postgres",
    url: config.dbUri,
    entities: [WorkloadEntity, MetalInstanceEntity, WorkloadEventEntity],
    synchronize,
    logging: false,
  });

  log.debug("Initializing database");
  await dataSource.initialize();

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
