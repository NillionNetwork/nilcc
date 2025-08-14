import { type ZodType, z } from "zod";
import { Account } from "#/account/account.dto";
import type { App } from "#/app";
import { PathsV1 } from "#/common/paths";
import type { AppBindings } from "#/env";
import {
  GetMetalInstanceResponse,
  type RegisterMetalInstanceRequest,
  type SubmitEventRequest,
} from "#/metal-instance/metal-instance.dto";
import {
  type CreateWorkloadRequest,
  CreateWorkloadResponse,
  GetWorkloadResponse,
  ListWorkloadsResponse,
  type WorkloadSystemLogsRequest,
  WorkloadSystemLogsResponse,
} from "#/workload/workload.dto";
import {
  type ListContainersRequest,
  ListContainersResponse,
  type WorkloadContainerLogsRequest,
  WorkloadContainerLogsResponse,
} from "#/workload-container/workload-container.dto";
import {
  type ListWorkloadEventsRequest,
  ListWorkloadEventsResponse,
} from "#/workload-event/workload-event.dto";

export type TestClientOptions = {
  app: App;
  bindings: AppBindings;
  apiToken: string;
};

export class TestClient {
  constructor(public _options: TestClientOptions) {}

  get app(): App {
    return this._options.app;
  }

  get bindings(): AppBindings {
    return this._options.bindings;
  }

  async request<T>(
    path: string,
    options: {
      method?: "GET" | "POST" | "PUT" | "DELETE";
      body?: T;
      params?: Record<string, string>;
      headers?: Record<string, string>;
    } = {},
  ): Promise<Response> {
    const { method = "GET", body, headers } = options;

    const requestHeaders: Record<string, string> = {
      ...this.extraHeaders(),
      ...headers,
    };

    if (body) {
      requestHeaders["Content-Type"] = "application/json";
    }

    return this.app.request(path, {
      method,
      headers: requestHeaders,
      ...(body && { body: JSON.stringify(body) }),
    });
  }

  extraHeaders(): Record<string, string> {
    return {
      "x-api-key": this._options.apiToken,
    };
  }
}

export class RequestPromise<T> {
  constructor(
    public promise: Promise<Response>,
    public schema: ZodType<T>,
  ) {}

  async submit(): Promise<T> {
    const response = await this.promise;
    const body = await response.json();
    if (response.status !== 200) {
      throw new Error(
        `Request failed with status: ${response.status}: ${JSON.stringify(body)}`,
      );
    }
    return this.schema.parse(body);
  }

  async status(): Promise<number> {
    const response = await this.promise;
    return response.status;
  }
}

export class AdminClient extends TestClient {
  getMetalInstance(id: string): RequestPromise<GetMetalInstanceResponse> {
    const promise = this.request(
      PathsV1.metalInstance.read.replace(":id", id),
      {
        method: "GET",
      },
    );
    return new RequestPromise(promise, GetMetalInstanceResponse);
  }

  deleteMetalInstance(id: string): RequestPromise<unknown> {
    const promise = this.request(PathsV1.metalInstance.delete, {
      method: "POST",
      body: { id },
    });
    return new RequestPromise(promise, z.unknown());
  }

  createAccount(name: string): RequestPromise<Account> {
    const promise = this.request(PathsV1.account.create, {
      method: "POST",
      body: { name },
    });
    return new RequestPromise(promise, Account);
  }

  listAccounts(): RequestPromise<Account[]> {
    const promise = this.request(PathsV1.account.list, {
      method: "GET",
    });
    return new RequestPromise(promise, Account.array());
  }

  getAccount(id: string): RequestPromise<Account> {
    const promise = this.request(PathsV1.account.read.replace(":id", id), {
      method: "GET",
    });
    return new RequestPromise(promise, Account);
  }
}

export class UserClient extends TestClient {
  createWorkload(
    body: CreateWorkloadRequest,
  ): RequestPromise<CreateWorkloadResponse> {
    const promise = this.request(PathsV1.workload.create, {
      method: "POST",
      body,
    });
    return new RequestPromise(promise, CreateWorkloadResponse);
  }

  getWorkload(id: string): RequestPromise<GetWorkloadResponse> {
    const promise = this.request(PathsV1.workload.read.replace(":id", id), {
      method: "GET",
    });
    return new RequestPromise(promise, GetWorkloadResponse);
  }

  listWorkloads(): RequestPromise<ListWorkloadsResponse> {
    const promise = this.request(PathsV1.workload.list, {
      method: "GET",
    });
    return new RequestPromise(promise, ListWorkloadsResponse);
  }

  deleteWorkload(id: string): RequestPromise<unknown> {
    const promise = this.request(PathsV1.workload.delete, {
      method: "POST",
      body: {
        id,
      },
    });
    return new RequestPromise(promise, z.unknown());
  }

  listEvents(workloadId: string): RequestPromise<ListWorkloadEventsResponse> {
    const body: ListWorkloadEventsRequest = { workloadId };
    const promise = this.request(PathsV1.workloadEvents.list, {
      method: "POST",
      body,
    });
    return new RequestPromise(promise, ListWorkloadEventsResponse);
  }

  listContainers(id: string): RequestPromise<ListContainersResponse> {
    const body: ListContainersRequest = { id };
    const promise = this.request(PathsV1.workloadContainers.list, {
      method: "POST",
      body,
    });
    return new RequestPromise(promise, ListContainersResponse);
  }

  containerLogs(
    id: string,
    container: string,
  ): RequestPromise<WorkloadContainerLogsResponse> {
    const body: WorkloadContainerLogsRequest = {
      id,
      container,
      stream: "stdout",
      tail: false,
      maxLines: 100,
    };
    const promise = this.request(PathsV1.workloadContainers.logs, {
      method: "POST",
      body,
    });
    return new RequestPromise(promise, WorkloadContainerLogsResponse);
  }

  logs(id: string): RequestPromise<WorkloadSystemLogsResponse> {
    const body: WorkloadSystemLogsRequest = {
      id,
      source: "cvm-agent",
      tail: false,
      maxLines: 100,
    };
    const promise = this.request(PathsV1.workload.logs, {
      method: "POST",
      body,
    });
    return new RequestPromise(promise, WorkloadSystemLogsResponse);
  }
}

export class MetalInstanceClient extends TestClient {
  register(body: RegisterMetalInstanceRequest): RequestPromise<unknown> {
    const promise = this.request(PathsV1.metalInstance.register, {
      method: "POST",
      body,
    });
    return new RequestPromise(promise, z.unknown());
  }

  heartbeat(id: string): RequestPromise<unknown> {
    const promise = this.request(PathsV1.metalInstance.heartbeat, {
      method: "POST",
      body: { id },
    });
    return new RequestPromise(promise, z.unknown());
  }

  submitEvent(request: SubmitEventRequest): RequestPromise<unknown> {
    const promise = this.request(PathsV1.workloadEvents.submit, {
      method: "POST",
      body: request,
    });
    return new RequestPromise(promise, z.unknown());
  }
}
