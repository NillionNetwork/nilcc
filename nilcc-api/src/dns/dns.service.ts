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
  registerCname(zone: string, domain: string, to: string): Promise<void>;
  removeDomain(zone: string, domain: string): Promise<void>;
}

export class Route53DnsService implements DnsService {
  protected readonly route53: Route53Client;
  readonly zoneIdCache: Record<string, string>;
  constructor(options?: {
    endpoint?: string;
    region?: string;
    credentials?: { accessKeyId: string; secretAccessKey: string };
  }) {
    this.route53 = new Route53Client([options]);
    this.zoneIdCache = {};
  }

  @mapError((e) => new RegisterCnameError(e))
  async registerCname(zone: string, domain: string, to: string): Promise<void> {
    const hostedZoneId = await this.findHostedZone(zone);
    const fullDomain = `${domain}.${zone}`;
    const command = new ChangeResourceRecordSetsCommand({
      HostedZoneId: hostedZoneId,
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
  async removeDomain(zone: string, domain: string): Promise<void> {
    const hostedZoneId = await this.findHostedZone(zone);
    const fullDomain = `${domain}.${zone}`;
    const domainData = await this.findDomain(hostedZoneId, fullDomain);
    if (!domainData) {
      throw Error(`Domain not found: ${fullDomain}`);
    }
    const command = new ChangeResourceRecordSetsCommand({
      HostedZoneId: hostedZoneId,
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

  private async findHostedZone(zone: string): Promise<string> {
    if (zone in this.zoneIdCache) {
      return this.zoneIdCache[zone];
    }
    const command = new ListHostedZonesCommand({});
    const response = await this.route53.send(command);
    if (!response.HostedZones) {
      throw Error(`Hosted zone not found: ${zone}`);
    }

    const hostedZone = response.HostedZones.find((z) => z.Name === `${zone}.`);
    if (!hostedZone || !hostedZone.Id) {
      throw Error(`Hosted zone not found: ${zone}`);
    }

    this.zoneIdCache[zone] = hostedZone.Id;
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
  initialized: boolean;
  workloadDnsDomain: string;

  constructor(workloadDnsDomain: string) {
    super({
      endpoint: "http://localhost:4566",
      region: "us-east-1", // LocalStack default region
      credentials: {
        accessKeyId: "test",
        secretAccessKey: "test",
      },
    });
    this.workloadDnsDomain = workloadDnsDomain;
    this.initialized = false;
  }

  override async registerCname(
    zone: string,
    domain: string,
    to: string,
  ): Promise<void> {
    this.initialized || (await this.initialize());
    return super.registerCname(zone, domain, to);
  }

  override async removeDomain(zone: string, domain: string): Promise<void> {
    this.initialized || (await this.initialize());
    return super.removeDomain(zone, domain);
  }

  private async initialize() {
    const callerReference = Math.random().toString();

    const createHostedZoneParams = {
      Name: this.workloadDnsDomain,
      CallerReference: callerReference,
    };

    const createHostedZoneCommand = new CreateHostedZoneCommand(
      createHostedZoneParams,
    );

    await this.route53.send(createHostedZoneCommand);

    this.initialized = true;
  }
}
