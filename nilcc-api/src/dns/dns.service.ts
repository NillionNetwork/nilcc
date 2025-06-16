import {
  ChangeResourceRecordSetsCommand,
  CreateHostedZoneCommand,
  ListHostedZonesCommand,
  ListResourceRecordSetsCommand,
  Route53Client,
} from "@aws-sdk/client-route-53";
import { mapError } from "#/common/errors";
import { RegisterCnameError, RemoveDomainError } from "#/dns/dns.errors";

export interface DnsService {
  registerCname(domain: string, to: string): Promise<void>;
  removeDomain(domain: string): Promise<void>;
}

export class Route53DnsService implements DnsService {
  protected readonly route53: Route53Client;
  readonly zoneId: string;
  readonly subdomain: string;

  protected constructor(
    subdomain: string,
    zoneId: string,
    route53: Route53Client,
  ) {
    this.route53 = route53;
    this.subdomain = subdomain;
    this.zoneId = zoneId;
  }

  static async create(
    subdomain: string,
    options?: {
      endpoint?: string;
      region?: string;
      credentials?: { accessKeyId: string; secretAccessKey: string };
    },
  ): Promise<Route53DnsService> {
    const route53 = new Route53Client([options]);
    const zoneId = await Route53DnsService.findHostedZone(route53, subdomain);
    return new Route53DnsService(subdomain, zoneId, route53);
  }

  @mapError((e) => new RegisterCnameError(e))
  async registerCname(domain: string, to: string): Promise<void> {
    const fullDomain = `${domain}.${this.subdomain}`;
    const command = new ChangeResourceRecordSetsCommand({
      HostedZoneId: this.zoneId,
      ChangeBatch: {
        Changes: [
          {
            Action: "CREATE",
            ResourceRecordSet: {
              Name: fullDomain,
              Type: "CNAME",
              TTL: 300,
              ResourceRecords: [{ Value: to }],
            },
          },
        ],
      },
    });
    await this.route53.send(command);
  }

  @mapError((e) => new RemoveDomainError(e))
  async removeDomain(domain: string): Promise<void> {
    const fullDomain = `${domain}.${this.subdomain}`;
    const domainData = await this.findDomain(this.zoneId, fullDomain);
    if (!domainData) {
      throw Error(`Domain not found: ${fullDomain}`);
    }
    const command = new ChangeResourceRecordSetsCommand({
      HostedZoneId: this.zoneId,
      ChangeBatch: {
        Changes: [
          {
            Action: "DELETE",
            ResourceRecordSet: domainData,
          },
        ],
      },
    });
    await this.route53.send(command);
  }

  private static async findHostedZone(
    route53: Route53Client,
    subdomain: string,
  ): Promise<string> {
    const command = new ListHostedZonesCommand({});
    const response = await route53.send(command);
    if (!response.HostedZones) {
      throw Error(`Hosted zone not found: ${subdomain}`);
    }

    const hostedZone = response.HostedZones.find(
      (z) => z.Name === `${subdomain}.`,
    );
    if (!hostedZone || !hostedZone.Id) {
      throw Error(`Hosted zone not found: ${subdomain}`);
    }

    return hostedZone.Id;
  }

  private async findDomain(zoneId: string, domain: string) {
    const command = new ListResourceRecordSetsCommand({
      HostedZoneId: zoneId,
      StartRecordName: `${domain}.`,
      StartRecordType: "CNAME",
      MaxItems: 1,
    });
    const response = await this.route53.send(command);
    const [recordset, ..._rest] = response.ResourceRecordSets || [null, null];
    return recordset;
  }
}

export class LocalStackDnsService extends Route53DnsService {
  static override async create(subdomain: string): Promise<Route53DnsService> {
    const route53 = new Route53Client([
      {
        endpoint: "http://localhost:4566",
        region: "us-east-1", // LocalStack default region
        credentials: {
          accessKeyId: "test",
          secretAccessKey: "test",
        },
      },
    ]);

    const zoneId = await LocalStackDnsService.createHostedZone(
      subdomain,
      route53,
    );
    return new LocalStackDnsService(subdomain, zoneId, route53);
  }

  private static async createHostedZone(
    subdomain: string,
    route53: Route53Client,
  ): Promise<string> {
    const callerReference = Math.random().toString();

    const createHostedZoneParams = {
      Name: subdomain,
      CallerReference: callerReference,
    };

    const createHostedZoneCommand = new CreateHostedZoneCommand(
      createHostedZoneParams,
    );

    const response = await route53.send(createHostedZoneCommand);
    if (!response.HostedZone || !response.HostedZone.Id) {
      throw new Error(`Failed to create hosted zone: ${subdomain}`);
    }
    return response.HostedZone.Id;
  }
}
