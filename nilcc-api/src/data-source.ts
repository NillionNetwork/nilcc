import "reflect-metadata";
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
