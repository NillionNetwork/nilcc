import type { QueryRunner, Repository } from "typeorm";
import { v4 as uuidv4 } from "uuid";
import type { Container } from "#/clients/nilcc-agent.client";
import {
  ContainerLogsError,
  CreateEntityError,
  FindEntityError,
  GetRepositoryError,
  InstancesNotAvailable,
  ListContainersError,
  ListWorkloadEventsError,
  mapError,
  RemoveEntityError,
  SubmitEventError,
} from "#/common/errors";
import { DockerComposeValidator } from "#/compose/validator";
import type { AppBindings } from "#/env";
import type {
  SubmitEventRequest,
  WorkloadEventKind,
} from "#/metal-instance/metal-instance.dto";
import type {
  CreateWorkloadRequest,
  ListContainersRequest,
  ListWorkloadEventsRequest,
  WorkloadContainerLogsRequest,
  WorkloadEvent,
} from "./workload.dto";
import { WorkloadEntity, WorkloadEventEntity } from "./workload.entity";

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
    request: CreateWorkloadRequest,
    tx: QueryRunner,
  ): Promise<WorkloadEntity> {
    const validator = new DockerComposeValidator();
    validator.validate(request.dockerCompose, request.serviceToExpose);

    const metalInstances =
      await bindings.services.metalInstance.findWithFreeResources(
        {
          cpus: request.cpus,
          memory: request.memory,
          disk: request.disk,
          gpus: request.gpus,
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
    const eventRepository = tx.manager.getRepository(WorkloadEventEntity);
    const now = new Date();
    const entity = repository.create({
      ...request,
      id: uuidv4(),
      metalInstance,
      createdAt: now,
      updatedAt: now,
    });
    const createdWorkload = await repository.save(entity);

    const event: WorkloadEventEntity = {
      id: uuidv4(),
      workload: entity,
      event: "created",
      timestamp: new Date(),
    };
    await eventRepository.save(event);

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
    tx: QueryRunner,
  ): Promise<void> {
    const workloadRepository = this.getRepository(bindings, tx);
    const eventRepository = tx.manager.getRepository(WorkloadEventEntity);
    const workload = await workloadRepository.findOneBy({
      id: request.workloadId,
    });
    if (workload === null) {
      throw new Error("workload not found");
    }
    switch (request.event.kind) {
      case "starting":
        workload.status = "starting";
        break;
      case "running":
        workload.status = "running";
        break;
      case "stopped":
        workload.status = "stopped";
        break;
      case "failedToStart":
        workload.status = "error";
        break;
    }
    let details: string | undefined;
    if (request.event.kind === "failedToStart") {
      details = request.event.error;
    }
    const event: WorkloadEventEntity = {
      id: uuidv4(),
      workload,
      event: request.event.kind,
      details,
      timestamp: new Date(),
    };
    await workloadRepository.save(workload);
    await eventRepository.save(event);
  }

  @mapError((e) => new ListWorkloadEventsError(e))
  async listEvents(
    bindings: AppBindings,
    request: ListWorkloadEventsRequest,
  ): Promise<Array<WorkloadEvent>> {
    const repository = this.getRepository(bindings);
    const workloads = await repository.find({
      where: { id: request.workloadId },
      relations: ["events"],
    });
    if (workloads.length === 0) {
      throw new Error("workload not found");
    }
    return workloads[0].events.map((event) => {
      let details: WorkloadEventKind;
      switch (event.event) {
        case "created":
          details = { kind: "created" };
          break;
        case "starting":
          details = { kind: "starting" };
          break;
        case "running":
          details = { kind: "running" };
          break;
        case "stopped":
          details = { kind: "stopped" };
          break;
        case "failedToStart":
          details = { kind: "failedToStart", error: event.details || "" };
          break;
      }
      return {
        id: event.id,
        details,
        timestamp: event.timestamp.toISOString(),
      };
    });
  }

  @mapError((e) => new ListContainersError(e))
  async listContainers(
    bindings: AppBindings,
    request: ListContainersRequest,
    tx?: QueryRunner,
  ): Promise<Array<Container>> {
    const workloadRepository = this.getRepository(bindings, tx);
    const workloads = await workloadRepository.find({
      where: { id: request.workloadId },
      relations: ["metalInstance"],
    });
    if (workloads.length !== 1) {
      throw new Error("workload not found");
    }
    const workload = workloads[0];
    return await bindings.services.nilccAgentClient.containers(
      workload.metalInstance,
      workload.id,
    );
  }

  @mapError((e) => new ContainerLogsError(e))
  async containerLogs(
    bindings: AppBindings,
    request: WorkloadContainerLogsRequest,
    tx?: QueryRunner,
  ): Promise<Array<string>> {
    const workloadRepository = this.getRepository(bindings, tx);
    const workloads = await workloadRepository.find({
      where: { id: request.workloadId },
      relations: ["metalInstance"],
    });
    if (workloads.length !== 1) {
      throw new Error("workload not found");
    }
    const workload = workloads[0];
    return await bindings.services.nilccAgentClient.containerLogs(
      workload.metalInstance,
      workload.id,
      request,
    );
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
    return `${workloadId}.${bindings.config.workloadsDnsDomain}`;
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
