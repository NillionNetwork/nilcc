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
  CreateWorkloadRequest,
  UpdateWorkloadRequest,
} from "./workload.api";
import { WorkloadEntity } from "./workload.entity";

function getRepository(
  bindings: AppBindings,
): E.Effect<Repository<WorkloadEntity>, GetRepositoryError> {
  try {
    return E.succeed(bindings.dataSource.getRepository(WorkloadEntity));
  } catch (e) {
    return E.fail(new GetRepositoryError({ cause: e }));
  }
}

function createWorkloadEntity(
  repo: Repository<WorkloadEntity>,
  workload: CreateWorkloadRequest,
): E.Effect<WorkloadEntity, CreateEntityError> {
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
  workload: CreateWorkloadRequest,
): E.Effect<WorkloadEntity, GetRepositoryError | CreateEntityError> {
  return pipe(
    getRepository(bindings),
    E.flatMap((repository) => createWorkloadEntity(repository, workload)),
  );
}

export function list(
  bindings: AppBindings,
): E.Effect<WorkloadEntity[], GetRepositoryError | FindEntityError> {
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
): E.Effect<WorkloadEntity | null, GetRepositoryError | FindEntityError> {
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
  payload: UpdateWorkloadRequest,
): E.Effect<boolean, GetRepositoryError | UpdateEntityError> {
  return pipe(
    getRepository(bindings),
    E.flatMap((repo) =>
      E.tryPromise({
        try: async () => {
          const updated = await repo.update(
            { id: payload.id },
            {
              name: payload.name,
              description: payload.description,
              tags: payload.tags,
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
