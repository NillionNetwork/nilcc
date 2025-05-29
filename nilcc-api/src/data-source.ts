import "reflect-metadata";
import type { Context, Next } from "hono";
import { DataSource } from "typeorm";
import { type EnvVars, FeatureFlag, hasFeatureFlag } from "#/env";
import { MetalInstanceEntity } from "#/metal-instance/metal-instance.entity";
import { WorkloadEntity } from "#/workload/workload.entity";

export async function buildDataSource(config: EnvVars): Promise<DataSource> {
  const synchronize = hasFeatureFlag(
    config.enabledFeatures,
    FeatureFlag.MIGRATIONS,
  );

  const dataSource = new DataSource({
    type: "postgres",
    url: config.dbUri,
    entities: [WorkloadEntity, MetalInstanceEntity],
    synchronize,
    logging: false,
  });

  await dataSource.initialize();

  return dataSource;
}

export function transactionMiddleware(dataSource: DataSource) {
  return async (c: Context, next: Next) => {
    const queryRunner = dataSource.createQueryRunner();

    try {
      await queryRunner.connect();
      await queryRunner.startTransaction();

      c.set("queryRunner", queryRunner);
      c.set("manager", queryRunner.manager);

      await next();

      await queryRunner.commitTransaction();
    } catch (error) {
      await queryRunner.rollbackTransaction();
      throw error;
    } finally {
      await queryRunner.release();
    }
  };
}
