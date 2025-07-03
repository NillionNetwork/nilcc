import type { ZodType } from "zod";
import type { App } from "#/app";
import { PathsV1 } from "#/common/paths";
import type { AppBindings } from "#/env";
import {
  GetMetalInstanceResponse,
  type RegisterMetalInstanceRequest,
} from "#/metal-instance/metal-instance.dto";
import {
  type CreateWorkloadRequest,
  CreateWorkloadResponse,
  GetWorkloadResponse,
  ListWorkloadsResponse,
  type UpdateWorkloadRequest,
} from "#/workload/workload.dto";

export type TestClientOptions = {
  app: App;
  bindings: AppBindings;
};

class TestClient {
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
    } = {},
  ): Promise<Response> {
    const { method = "GET", body } = options;

    const headers: Record<string, string> = this.extraHeaders();

    if (body) {
      headers["Content-Type"] = "application/json";
    }

    return this.app.request(path, {
      method,
      headers,
      ...(body && { body: JSON.stringify(body) }),
    });
  }

  extraHeaders(): Record<string, string> {
    // This method can be overridden to add extra headers if needed
    return {};
  }
}

class ParseableResponse<T> {
  constructor(
    public response: Response,
    public schema: ZodType<T>,
  ) {}

  async parse_body(): Promise<T> {
    const response = await this.response.json();
    if (!this.response.ok) {
      throw new Error(
        `Request failed with status ${this.response.status}: ${JSON.stringify(
          response,
        )}`,
      );
    }
    return this.schema.parse(response);
  }
}

export class UserClient extends TestClient {
  override extraHeaders(): Record<string, string> {
    return {
      "x-api-key": this.bindings.config.userApiKey,
    };
  }

  async createWorkload(
    body: CreateWorkloadRequest,
  ): Promise<ParseableResponse<CreateWorkloadResponse>> {
    const response = await this.request(PathsV1.workload.create, {
      method: "POST",
      body,
    });
    return new ParseableResponse<CreateWorkloadResponse>(
      response,
      CreateWorkloadResponse,
    );
  }

  async getWorkload(params: {
    id: string;
  }): Promise<ParseableResponse<GetWorkloadResponse>> {
    const response = await this.request(
      PathsV1.workload.read.replace(":id", params.id),
      {
        method: "GET",
      },
    );
    return new ParseableResponse<GetWorkloadResponse>(
      response,
      GetWorkloadResponse,
    );
  }

  async listWorkloads(): Promise<ParseableResponse<ListWorkloadsResponse>> {
    const response = await this.request(PathsV1.workload.list, {
      method: "GET",
    });
    return new ParseableResponse<ListWorkloadsResponse>(
      response,
      ListWorkloadsResponse,
    );
  }

  async updateWorkload(body: UpdateWorkloadRequest): Promise<Response> {
    return await this.request(PathsV1.workload.update, {
      method: "PUT",
      body,
    });
  }

  async deleteWorkload(params: { id: string }): Promise<Response> {
    return this.request(PathsV1.workload.remove.replace(":id", params.id), {
      method: "DELETE",
      params,
    });
  }

  async getMetalInstance(params: { id: string }) {
    const response = await this.request(
      PathsV1.metalInstance.read.replace(":id", params.id),
      {
        method: "GET",
      },
    );
    return new ParseableResponse<GetMetalInstanceResponse>(
      response,
      GetMetalInstanceResponse,
    );
  }
}

export class MetalInstanceClient extends TestClient {
  override extraHeaders(): Record<string, string> {
    return {
      "x-api-key": this.bindings.config.metalInstanceApiKey,
    };
  }

  async register(body: RegisterMetalInstanceRequest): Promise<Response> {
    return await this.request(PathsV1.metalInstance.register, {
      method: "POST",
      body,
    });
  }
}
