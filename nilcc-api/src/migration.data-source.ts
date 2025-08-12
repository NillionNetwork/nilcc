import "./effects";

import { buildDataSource } from "./data-source";
import { parseConfigFromEnv } from "./env";

const config = parseConfigFromEnv({});
export default buildDataSource(config);
