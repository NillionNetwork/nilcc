import type { QueryRunner, Repository } from "typeorm";
import { EntityAlreadyExists, isUniqueConstraint } from "#/common/errors";
import type { AppBindings } from "#/env";
import type { EnableArtifactRequest } from "./artifact.dto";
import { ArtifactEntity } from "./artifact.entity";

export class ArtifactService {
  getRepository(
    bindings: AppBindings,
    tx?: QueryRunner,
  ): Repository<ArtifactEntity> {
    if (tx) {
      return tx.manager.getRepository(ArtifactEntity);
    }
    return bindings.dataSource.getRepository(ArtifactEntity);
  }

  async enable(
    bindings: AppBindings,
    request: EnableArtifactRequest,
  ): Promise<ArtifactEntity> {
    const repository = this.getRepository(bindings);
    const metadata = await bindings.services.artifactsClient.fetchMetadata(
      request.version,
    );
    try {
      return await repository.save({
        version: request.version,
        builtAt: new Date(metadata.built_at * 1000),
      });
    } catch (e: unknown) {
      if (isUniqueConstraint(e)) {
        throw new EntityAlreadyExists("artifact");
      }
      throw e;
    }
  }

  async list(bindings: AppBindings): Promise<ArtifactEntity[]> {
    const repository = this.getRepository(bindings);
    return await repository.find();
  }

  async remove(bindings: AppBindings, version: string): Promise<void> {
    const repository = this.getRepository(bindings);
    await repository.delete({ version });
  }
}
