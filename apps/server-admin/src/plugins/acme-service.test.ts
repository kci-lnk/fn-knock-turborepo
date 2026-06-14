import assert from "node:assert/strict";
import test from "node:test";
import {
  translate,
  type LocaleCode,
  type MessageParams,
} from "../../../../packages/i18n/src";
import { AcmeService } from "./acme/AcmeService";

const translateWithLocale =
  (locale: LocaleCode) => (key: string, params?: MessageParams) =>
    translate(locale, key, params);

test("ACME service state message uses provided translator", () => {
  const service = new AcmeService();

  assert.equal(service.getState().messageKey, "waiting");
  assert.equal(
    service.getLocalizedState(translateWithLocale("ko-KR")).message,
    "조치를 기다리는 중",
  );
});
