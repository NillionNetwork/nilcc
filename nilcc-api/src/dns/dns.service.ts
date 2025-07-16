import {
  ChangeResourceRecordSetsCommand,
  CreateHostedZoneCommand,
  ListHostedZonesCommand,
  ListResourceRecordSetsCommand,
  Route53Client,
  type RRType,
} from "@aws-sdk/client-route-53";
import { mapError } from "#/common/errors";
import { CreateRecordError, DeleteRecordError } from "#/dns/dns.errors";

export interface DnsService {
  createRecord(domain: string, to: string, recordType: RRType): Promise<void>;
  deleteRecord(domain: string, recordType: RRType): Promise<void>;
}

export class Route53DnsService implements DnsService {
  protected readonly route53: Route53Client;
  readonly zone: string;
  readonly zoneId: string;
  readonly subdomain: string;

  protected constructor(
    zone: string,
    subdomain: string,
    zoneId: string,
    route53: Route53Client,
  ) {
    this.zone = zone;
    this.subdomain = subdomain;
    this.route53 = route53;
    this.zoneId = zoneId;
  }

  static async create(
    zone: string,
    subdomain: string,
    options?: {
      endpoint?: string;
      region?: string;
      credentials?: { accessKeyId: string; secretAccessKey: string };
    },
  ): Promise<Route53DnsService> {
    let route53: Route53Client;
    if (options) {
      route53 = new Route53Client(options);
    } else {
      route53 = new Route53Client();
    }
    if (!subdomain.endsWith(zone)) {
      throw new Error(`${subdomain} isn't a subdomain of ${zone}`);
    }

    const zoneId = await Route53DnsService.findHostedZone(route53, zone);
    return new Route53DnsService(zone, subdomain, zoneId, route53);
  }

  @mapError((e) => new CreateRecordError(e))
  async createRecord(
    domain: string,
    to: string,
    recordType: RRType,
  ): Promise<void> {
    const fullDomain = `${domain}.${this.subdomain}`;
    const command = new ChangeResourceRecordSetsCommand({
      HostedZoneId: this.zoneId,
      ChangeBatch: {
        Changes: [
          {
            Action: "CREATE",
            ResourceRecordSet: {
              Name: fullDomain,
              Type: recordType,
              TTL: 300,
              ResourceRecords: [{ Value: to }],
            },
          },
        ],
      },
    });
    await this.route53.send(command);
  }

  @mapError((e) => new DeleteRecordError(e))
  async deleteRecord(domain: string, recordType: RRType): Promise<void> {
    const fullDomain = `${domain}.${this.subdomain}`;
    const domainData = await this.findDomain(
      this.zoneId,
      fullDomain,
      recordType,
    );
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

  private async findDomain(zoneId: string, domain: string, recordType: RRType) {
    const command = new ListResourceRecordSetsCommand({
      HostedZoneId: zoneId,
      StartRecordName: `${domain}.`,
      StartRecordType: recordType,
      MaxItems: 1,
    });
    const response = await this.route53.send(command);
    const [recordset, ..._rest] = response.ResourceRecordSets || [null, null];
    return recordset;
  }
}

export class LocalStackDnsService extends Route53DnsService {
  static override async create(
    zone: string,
    subdomain: string,
  ): Promise<Route53DnsService> {
    const route53 = new Route53Client({
      endpoint: process.env.APP_LOCALSTACK_URI,
      region: "us-east-1", // LocalStack default region
      credentials: {
        accessKeyId: "test",
        secretAccessKey: "test",
      },
    });

    const zoneId = await LocalStackDnsService.createHostedZone(
      subdomain,
      route53,
    );
    return new LocalStackDnsService(zone, subdomain, zoneId, route53);
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
