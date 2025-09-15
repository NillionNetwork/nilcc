import type { QueryRunner, Repository } from "typeorm";
import { v4 as uuidv4 } from "uuid";
import type { AccountEntity } from "#/account/account.entity";
import type {
  Container,
  SystemStatsResponse,
} from "#/clients/nilcc-agent.client";
import {
  AccessDenied,
  EntityNotFound,
  InvalidWorkloadTier,
  NoInstancesAvailable,
  NotEnoughCredits,
} from "#/common/errors";
import type { AppBindings } from "#/env";
import type {
  ListContainersRequest,
  WorkloadContainerLogsRequest,
} from "#/workload-container/workload-container.dto";
import type {
  ListWorkloadEventsRequest,
  SubmitEventRequest,
  WorkloadEvent,
  WorkloadEventKind,
} from "#/workload-event/workload-event.dto";
import { WorkloadTierEntity } from "#/workload-tier/workload-tier.entity";
import type {
  CreateWorkloadRequest,
  StatsRequest,
  WorkloadSystemLogsRequest,
} from "./workload.dto";
import { WorkloadEntity, WorkloadEventEntity } from "./workload.entity";

const MINIMUM_EXECUTION_DURATION: number = 5;

export class WorkloadService {
  getRepository(
    bindings: AppBindings,
    tx: QueryRunner,
  ): Repository<WorkloadEntity> {
    if (tx) {
      return tx.manager.getRepository(WorkloadEntity);
    }
    return bindings.dataSource.getRepository(WorkloadEntity);
  }

  async create(
    bindings: AppBindings,
    request: CreateWorkloadRequest,
    account: AccountEntity,
    tx: QueryRunner,
  ): Promise<WorkloadEntity> {
    const tier = await tx.manager.getRepository(WorkloadTierEntity).findOneBy({
      cpus: request.cpus,
      memory: request.memory,
      disk: request.disk,
      gpus: request.gpus,
    });
    if (tier === null) {
      throw new InvalidWorkloadTier();
    }

    // Make sure the account has enough credits to run this and all the existing workoads for 5 minutes.
    const totalAccountSpend =
      await bindings.services.account.getAccountSpending(bindings, account.id);
    if (
      (totalAccountSpend + tier.cost) * MINIMUM_EXECUTION_DURATION >
      account.credits
    ) {
      throw new NotEnoughCredits();
    }
    const repository = this.getRepository(bindings, tx);

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
      throw new NoInstancesAvailable();
    }

    // Get the first instance after sorting by id
    const metalInstance = metalInstances.sort((a, b) =>
      a.id.localeCompare(b.id),
    )[0];

    // Assign the first available metal instance to the workload
    const eventRepository = tx.manager.getRepository(WorkloadEventEntity);
    const now = bindings.services.time.getTime();
    const entity = repository.create({
      ...request,
      publicContainerName: request.publicContainerName,
      publicContainerPort: request.publicContainerPort,
      id: uuidv4(),
      metalInstance,
      account,
      domain: request.domain,
      createdAt: now,
      updatedAt: now,
      creditRate: tier.cost,
    });
    const createdWorkload = await repository.save(entity);

    const event: WorkloadEventEntity = {
      id: uuidv4(),
      workload: entity,
      event: "created",
      timestamp: now,
    };
    await eventRepository.save(event);

    const domain =
      request.domain || `${entity.id}.${bindings.config.workloadsDnsDomain}`;
    await bindings.services.nilccAgentClient.createWorkload(
      metalInstance,
      entity,
      domain,
    );
    if (request.domain === undefined) {
      await this.createCnameForWorkload(
        bindings,
        createdWorkload.id,
        metalInstance.id,
      );
    }
    return createdWorkload;
  }

  async list(
    bindings: AppBindings,
    account: AccountEntity,
    tx: QueryRunner,
  ): Promise<WorkloadEntity[]> {
    const repository = this.getRepository(bindings, tx);
    return await repository.find({
      where: { account },
      relations: ["account", "metalInstance"],
    });
  }

  async read(
    bindings: AppBindings,
    workloadId: string,
    account: AccountEntity,
    tx: QueryRunner,
  ): Promise<WorkloadEntity | null> {
    const repository = this.getRepository(bindings, tx);
    return await this.findWorkload(repository, workloadId, account, [
      "metalInstance",
    ]);
  }

  async remove(
    bindings: AppBindings,
    workloadId: string,
    account: AccountEntity,
    tx: QueryRunner,
  ): Promise<void> {
    const repository = this.getRepository(bindings, tx);
    const workload = await this.findWorkload(repository, workloadId, account, [
      "metalInstance",
    ]);
    if (workload === null) {
      throw new EntityNotFound("workload");
    }

    await repository.delete({ id: workloadId });
    if (workload.domain === undefined) {
      await this.removeCnameForWorkload(bindings, workloadId);
    }
    await bindings.services.nilccAgentClient.deleteWorkload(
      workload.metalInstance,
      workloadId,
    );
  }

  async restart(
    bindings: AppBindings,
    workloadId: string,
    account: AccountEntity,
    tx: QueryRunner,
  ): Promise<void> {
    const repository = this.getRepository(bindings, tx);
    const workload = await this.findWorkload(repository, workloadId, account, [
      "metalInstance",
    ]);
    if (workload === null) {
      throw new EntityNotFound("workload");
    }
    // Don't allow restarting if we don't have enough credits
    if (account.credits === 0) {
      throw new NotEnoughCredits();
    }

    await bindings.services.nilccAgentClient.restartWorkload(
      workload.metalInstance,
      workloadId,
    );
  }

  async start(
    bindings: AppBindings,
    workloadId: string,
    account: AccountEntity,
    tx: QueryRunner,
  ): Promise<void> {
    const repository = this.getRepository(bindings, tx);
    const workload = await this.findWorkload(repository, workloadId, account, [
      "metalInstance",
    ]);
    if (workload === null) {
      throw new EntityNotFound("workload");
    }
    // Don't allow starting if we don't have enough credits
    if (account.credits === 0) {
      throw new NotEnoughCredits();
    }

    await bindings.services.nilccAgentClient.startWorkload(
      workload.metalInstance,
      workloadId,
    );
  }

  async stop(
    bindings: AppBindings,
    workloadId: string,
    account: AccountEntity,
    tx: QueryRunner,
  ): Promise<void> {
    const repository = this.getRepository(bindings, tx);
    const workload = await this.findWorkload(repository, workloadId, account, [
      "metalInstance",
    ]);
    if (workload === null) {
      throw new EntityNotFound("workload");
    }

    await bindings.services.nilccAgentClient.stopWorkload(
      workload.metalInstance,
      workloadId,
    );
  }

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
      throw new EntityNotFound("workload");
    }
    switch (request.event.kind) {
      case "starting":
      case "vmRestarted":
      case "forcedRestart":
        workload.status = "starting";
        break;
      case "awaitingCert":
        workload.status = "awaitingCert";
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
      case "warning":
        // We don't want a state change for warnings
        break;
    }
    let details: string | undefined;
    if (request.event.kind === "failedToStart") {
      details = request.event.error;
    } else if (request.event.kind === "warning") {
      details = request.event.message;
    }
    const event: WorkloadEventEntity = {
      id: uuidv4(),
      workload,
      event: request.event.kind,
      details,
      timestamp: new Date(request.timestamp),
    };
    await workloadRepository.save(workload);
    await eventRepository.save(event);
  }

  async listEvents(
    bindings: AppBindings,
    request: ListWorkloadEventsRequest,
    account: AccountEntity,
    tx: QueryRunner,
  ): Promise<Array<WorkloadEvent>> {
    const repository = this.getRepository(bindings, tx);
    const workload = await this.findWorkload(
      repository,
      request.workloadId,
      account,
      ["events"],
    );
    if (workload === null) {
      throw new EntityNotFound("workload");
    }
    return workload.events.map((event) => {
      let details: WorkloadEventKind;
      if (event.event === "failedToStart") {
        details = { kind: "failedToStart", error: event.details || "" };
      } else if (event.event === "warning") {
        details = { kind: "warning", message: event.details || "" };
      } else {
        details = { kind: event.event };
      }
      return {
        eventId: event.id,
        details,
        timestamp: event.timestamp.toISOString(),
      };
    });
  }

  async systemLogs(
    bindings: AppBindings,
    request: WorkloadSystemLogsRequest,
    account: AccountEntity,
    tx: QueryRunner,
  ): Promise<Array<string>> {
    const repository = this.getRepository(bindings, tx);
    const workload = await this.findWorkload(
      repository,
      request.workloadId,
      account,
      ["metalInstance"],
    );
    if (workload === null) {
      throw new EntityNotFound("workload");
    }
    return await bindings.services.nilccAgentClient.systemLogs(
      workload.metalInstance,
      workload.id,
      request,
    );
  }

  async systemStats(
    bindings: AppBindings,
    request: StatsRequest,
    account: AccountEntity,
    tx: QueryRunner,
  ): Promise<SystemStatsResponse> {
    const repository = this.getRepository(bindings, tx);
    const workload = await this.findWorkload(
      repository,
      request.workloadId,
      account,
      ["metalInstance"],
    );
    if (workload === null) {
      throw new EntityNotFound("workload");
    }
    return await bindings.services.nilccAgentClient.systemStats(
      workload.metalInstance,
      workload.id,
    );
  }

  async listContainers(
    bindings: AppBindings,
    request: ListContainersRequest,
    account: AccountEntity,
    tx: QueryRunner,
  ): Promise<Array<Container>> {
    const repository = this.getRepository(bindings, tx);
    const workload = await this.findWorkload(
      repository,
      request.workloadId,
      account,
      ["metalInstance"],
    );
    if (workload === null) {
      throw new EntityNotFound("workload");
    }
    return await bindings.services.nilccAgentClient.containers(
      workload.metalInstance,
      workload.id,
    );
  }

  async containerLogs(
    bindings: AppBindings,
    request: WorkloadContainerLogsRequest,
    account: AccountEntity,
    tx: QueryRunner,
  ): Promise<Array<string>> {
    const repository = this.getRepository(bindings, tx);
    const workload = await this.findWorkload(
      repository,
      request.workloadId,
      account,
      ["metalInstance"],
    );
    if (workload === null) {
      throw new EntityNotFound("workload");
    }
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

  validateAccount(expected: AccountEntity, actual: AccountEntity): void {
    if (expected.id !== actual.id) {
      throw new AccessDenied();
    }
  }

  async findWorkload(
    repository: Repository<WorkloadEntity>,
    workloadId: string,
    account: AccountEntity,
    relations: string[] = [],
  ): Promise<WorkloadEntity | null> {
    const workloads = await repository.find({
      where: {
        id: workloadId,
      },
      relations: ["account", ...relations],
    });
    if (workloads.length === 0) {
      return null;
    }
    const workload = workloads[0];
    this.validateAccount(account, workload.account);
    return workload;
  }
}
