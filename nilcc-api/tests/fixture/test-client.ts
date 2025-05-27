import type { ZodType } from "zod";
import type { App } from "#/app";
import { PathsV1 } from "#/common/paths";
import {
  ApiResponseCreateWorkloadSchema,
  ApiResponseListWorkloadsSchema,
  type CreateWorkloadRequest,
  type CreateWorkloadResponse,
  type GetWorkloadResponse,
  type ListWorkloadsResponse,
  type UpdateWorkloadRequest,
} from "#/workload/workload.api";

export type TestClientOptions = {
  app: App;
};

class TestClient {
  constructor(public _options: TestClientOptions) {}

  get app(): App {
    return this._options.app;
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

    const headers: Record<string, string> = {};

    if (body) {
      headers["Content-Type"] = "application/json";
    }

    return this.app.request(path, {
      method,
      headers,
      ...(body && { body: JSON.stringify(body) }),
    });
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

export class WorkloadClient extends TestClient {
  async createWorkload(
    body: CreateWorkloadRequest,
  ): Promise<ParseableResponse<CreateWorkloadResponse>> {
    const response = await this.request(PathsV1.workload.create, {
      method: "POST",
      body,
    });
    return new ParseableResponse<CreateWorkloadResponse>(
      response,
      ApiResponseCreateWorkloadSchema,
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
      ApiResponseCreateWorkloadSchema,
    );
  }

  async listWorkloads(): Promise<ParseableResponse<ListWorkloadsResponse>> {
    const response = await this.request(PathsV1.workload.list, {
      method: "GET",
    });
    return new ParseableResponse<ListWorkloadsResponse>(
      response,
      ApiResponseListWorkloadsSchema,
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
}
