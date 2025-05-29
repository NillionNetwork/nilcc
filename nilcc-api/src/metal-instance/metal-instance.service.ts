import { Effect as E, pipe } from "effect";
import type { Repository } from "typeorm";
import {
  CreateEntityError,
  FindEntityError,
  GetRepositoryError,
  RemoveEntityError,
  UpdateEntityError,
} from "#/common/errors";
import type { AppBindings } from "#/env";
import type {
  CreateMetalInstanceRequest,
  UpdateMetalInstanceRequest,
} from "./metal-instance.dto";
import { MetalInstanceEntity } from "./metal-instance.entity";

function getRepository(
  bindings: AppBindings,
): E.Effect<Repository<MetalInstanceEntity>, GetRepositoryError> {
  try {
    return E.succeed(bindings.dataSource.getRepository(MetalInstanceEntity));
  } catch (e) {
    return E.fail(new GetRepositoryError({ cause: e }));
  }
}

function createMetalInstanceEntity(
  repo: Repository<MetalInstanceEntity>,
  workload: CreateMetalInstanceRequest,
): E.Effect<MetalInstanceEntity, CreateEntityError> {
  return E.tryPromise({
    try: async () => {
      const now = new Date();
      const entity = repo.create({
        ...workload,
        createdAt: now,
        updatedAt: now,
      });
      return repo.save(entity);
    },
    catch: (e) => new CreateEntityError({ cause: e }),
  });
}

export function create(
  bindings: AppBindings,
  workload: CreateMetalInstanceRequest,
): E.Effect<MetalInstanceEntity, GetRepositoryError | CreateEntityError> {
  return pipe(
    getRepository(bindings),
    E.flatMap((repository) => createMetalInstanceEntity(repository, workload)),
  );
}

export function list(
  bindings: AppBindings,
): E.Effect<MetalInstanceEntity[], GetRepositoryError | FindEntityError> {
  return pipe(
    getRepository(bindings),
    E.flatMap((repository) =>
      E.tryPromise({
        try: async () => repository.find(),
        catch: (e) => new FindEntityError({ cause: e }),
      }),
    ),
  );
}

export function read(
  bindings: AppBindings,
  workloadId: string,
): E.Effect<MetalInstanceEntity | null, GetRepositoryError | FindEntityError> {
  return pipe(
    getRepository(bindings),
    E.flatMap((repo) =>
      E.tryPromise({
        try: async () => repo.findOneBy({ id: workloadId }),
        catch: (e) => new FindEntityError({ cause: e }),
      }),
    ),
  );
}

export function update(
  bindings: AppBindings,
  payload: UpdateMetalInstanceRequest,
): E.Effect<boolean, GetRepositoryError | UpdateEntityError> {
  return pipe(
    getRepository(bindings),
    E.flatMap((repo) =>
      E.tryPromise({
        try: async () => {
          const updated = await repo.update(
            { id: payload.id },
            {
              hostname: payload.hostname,
              memory: payload.memory,
              cpu: payload.cpu,
              gpu: payload.gpu,
              gpuModel: payload.gpuModel,
              ipAddress: payload.ipAddress,
              updatedAt: new Date(),
            },
          );
          return updated.affected ? updated.affected > 0 : false;
        },
        catch: (e) => new UpdateEntityError({ cause: e }),
      }),
    ),
  );
}

export function remove(
  bindings: AppBindings,
  workloadId: string,
): E.Effect<boolean, GetRepositoryError | RemoveEntityError> {
  return pipe(
    getRepository(bindings),
    E.flatMap((repo) =>
      E.tryPromise({
        try: async () => {
          const result = await repo.delete({ id: workloadId });
          return result.affected ? result.affected > 0 : false;
        },
        catch: (e) => new RemoveEntityError({ cause: e }),
      }),
    ),
  );
}
