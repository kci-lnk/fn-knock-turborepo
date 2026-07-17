import type {
  FrpcInstanceStatus,
  FrpcInstanceSummary,
  FrpcInstancesOverview,
} from "@/lib/api";
import { extractVisualFieldsFromToml } from "../../../lib/frpc-config-editor";

export const summarizeFrpcContent = (
  raw: string,
  localPort: string,
): FrpcInstanceSummary => {
  try {
    const fields = extractVisualFieldsFromToml(raw, { localPort });
    return {
      serverAddr: fields.serverAddr,
      serverPort: fields.serverPort,
      localPort: fields.localPort,
      remotePort: fields.remotePort,
    };
  } catch {
    return {
      serverAddr: "",
      serverPort: "7000",
      localPort,
      remotePort: "",
    };
  }
};

export const replaceFrpcOverviewItem = (
  overview: FrpcInstancesOverview,
  item: FrpcInstanceStatus,
): FrpcInstancesOverview => ({
  ...overview,
  items: overview.items.map((current) =>
    current.id === item.id ? item : current,
  ),
  runningCount: overview.items.reduce(
    (count, current) =>
      count +
      (current.id === item.id
        ? Number(item.running)
        : Number(current.running)),
    0,
  ),
});
