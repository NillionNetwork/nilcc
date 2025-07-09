import type { QueryRunner, Repository } from "typeorm";
import {
  CreateEntityError,
  FindEntityError,
  GetRepositoryError,
  InstancesNotAvailable,
  mapError,
  RemoveEntityError,
  SubmitEventError,
} from "#/common/errors";
import { DockerComposeValidator } from "#/compose/validator";
import type { AppBindings } from "#/env";
import type { SubmitEventRequest } from "#/metal-instance/metal-instance.dto";
import type { CreateWorkloadRequest } from "./workload.dto";
import { WorkloadEntity } from "./workload.entity";

export class WorkloadService {
  @mapError((e) => new GetRepositoryError(e))
  getRepository(
    bindings: AppBindings,
    tx?: QueryRunner,
  ): Repository<WorkloadEntity> {
    if (tx) {
      return tx.manager.getRepository(WorkloadEntity);
    }
    return bindings.dataSource.getRepository(WorkloadEntity);
  }

  @mapError((e) => new CreateEntityError(WorkloadEntity, e))
  async create(
    bindings: AppBindings,
    workload: CreateWorkloadRequest,
    tx: QueryRunner,
  ): Promise<WorkloadEntity> {
    const validator = new DockerComposeValidator();
    validator.validate(workload.dockerCompose, workload.serviceToExpose);

    const metalInstances =
      await bindings.services.metalInstance.findWithFreeResources(
        {
          cpus: workload.cpus,
          memory: workload.memory,
          disk: workload.disk,
          gpus: workload.gpus,
        },
        bindings,
        tx,
      );

    if (metalInstances.length === 0) {
      throw new InstancesNotAvailable();
    }

    const metalInstance =
      metalInstances[Math.floor(Math.random() * metalInstances.length)];

    // Assign the first available metal instance to the workload
    const repository = this.getRepository(bindings, tx);
    const now = new Date();
    const entity = repository.create({
      ...workload,
      metalInstance,
      createdAt: now,
      updatedAt: now,
    });
    const createdWorkload = await repository.save(entity);
    const domain = await this.createCnameForWorkload(
      bindings,
      createdWorkload.id,
      metalInstance.id,
    );
    await bindings.services.nilccAgentClient.createWorkload(
      metalInstance,
      entity,
      domain,
    );
    return createdWorkload;
  }

  @mapError((e) => new FindEntityError(WorkloadEntity, e))
  async list(bindings: AppBindings): Promise<WorkloadEntity[]> {
    const repository = this.getRepository(bindings);
    return await repository.find();
  }

  @mapError((e) => new FindEntityError(WorkloadEntity, e))
  async read(
    bindings: AppBindings,
    workloadId: string,
  ): Promise<WorkloadEntity | null> {
    const repository = this.getRepository(bindings);
    return await repository.findOneBy({ id: workloadId });
  }

  @mapError((e) => new RemoveEntityError(WorkloadEntity, e))
  async remove(bindings: AppBindings, workloadId: string): Promise<boolean> {
    const workloadRepository = this.getRepository(bindings);
    const workloads = await workloadRepository.find({
      where: { id: workloadId },
      relations: ["metalInstance"],
    });
    if (workloads.length === 0) {
      return false;
    }
    const workload = workloads[0];
    await workloadRepository.delete({ id: workloadId });
    await this.removeCnameForWorkload(bindings, workloadId);
    await bindings.services.nilccAgentClient.deleteWorkload(
      workload.metalInstance,
      workloadId,
    );
    return true;
  }

  @mapError((e) => new SubmitEventError(e))
  async submitEvent(
    bindings: AppBindings,
    request: SubmitEventRequest,
    tx?: QueryRunner,
  ): Promise<void> {
    const workloadRepository = this.getRepository(bindings, tx);
    const workload = await workloadRepository.findOneBy({
      id: request.workloadId,
    });
    if (workload === null) {
      throw new Error("workload not found");
    }
    switch (request.event.kind) {
      case "started":
        workload.status = "running";
        break;
      case "stopped":
        workload.status = "stopped";
        break;
      case "failedToStart":
        workload.status = "error";
        break;
    }
    await workloadRepository.save(workload);
  }

  async createCnameForWorkload(
    bindings: AppBindings,
    workloadId: string,
    metalInstanceId: string,
  ): Promise<string> {
    const metalInstanceDomain = `${metalInstanceId}.${bindings.config.metalInstancesDnsDomain}`;
    await bindings.services.dns.workloads.createRecord(
      workloadId,
      metalInstanceDomain,
      "CNAME",
    );
    return metalInstanceDomain;
  }

  async removeCnameForWorkload(
    bindings: AppBindings,
    workloadId: string,
  ): Promise<void> {
    return await bindings.services.dns.workloads.deleteRecord(
      workloadId,
      "CNAME",
    );
  }
}
