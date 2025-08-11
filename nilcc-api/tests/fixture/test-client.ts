import { type ZodType, z } from "zod";
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
} from "#/workload/workload.dto";
import {
  type ListWorkloadEventsRequest,
  ListWorkloadEventsResponse,
} from "#/workload-event/workload-event.dto";

export type TestClientOptions = {
  app: App;
  bindings: AppBindings;
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
    // This method can be overridden to add extra headers if needed
    return {};
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
  override extraHeaders(): Record<string, string> {
    return {
      "x-api-key": this.bindings.config.adminApiKey,
    };
  }

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
}

export class UserClient extends TestClient {
  override extraHeaders(): Record<string, string> {
    return {
      "x-api-key": this.bindings.config.userApiKey,
    };
  }

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

  getWorkloadEvents(
    workloadId: string,
  ): RequestPromise<ListWorkloadEventsResponse> {
    const body: ListWorkloadEventsRequest = { workloadId };
    const promise = this.request(PathsV1.workloadEvents.list, {
      method: "POST",
      body,
    });
    return new RequestPromise(promise, ListWorkloadEventsResponse);
  }
}

export class MetalInstanceClient extends TestClient {
  override extraHeaders(): Record<string, string> {
    return {
      "x-api-key": this.bindings.config.metalInstanceApiKey,
    };
  }

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
