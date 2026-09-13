import { browserT } from "@fn-knock/i18n/vue/admin";
import type { IpLocationSnapshot } from "../types";

export const getIpLocationText = (snapshot: IpLocationSnapshot | null) => {
  if (snapshot?.location) return snapshot.location;

  if (snapshot?.status === "queued" || snapshot?.status === "processing") {
    return browserT("admin.hostActiveIps.resolving");
  }

  if (snapshot?.status === "skipped") {
    return browserT("admin.hostActiveIps.privateAddress");
  }

  if (snapshot?.status === "failed") {
    return browserT("admin.hostActiveIps.unavailable");
  }

  return browserT("admin.hostActiveIps.unavailable");
};
