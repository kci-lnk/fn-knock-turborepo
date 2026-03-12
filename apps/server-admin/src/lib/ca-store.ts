import { join } from "node:path";
import { promises as fs } from "node:fs";
import { X509Certificate, randomBytes } from "node:crypto";
import forge from "node-forge";
import { dataPath } from './AppDirManager';
const DATA_DIR = dataPath;
const CA_DIR = join(DATA_DIR, "ssl");
const CA_CERT_PATH = join(CA_DIR, "rootCA.pem");
const CA_KEY_PATH = join(CA_DIR, "rootCA.key.pem");
const CA_HOSTS_PATH = join(CA_DIR, "hosts.json");

export type CAInfo = {
    subject: string;
    issuer: string;
    validFrom: string;
    validTo: string;
    serialNumber: string;
};

async function ensureDir(p: string) {
    await fs.mkdir(p, { recursive: true });
}

async function readHostsRaw(): Promise<string[]> {
    try {
        const data = await fs.readFile(CA_HOSTS_PATH, "utf-8");
        const parsed = JSON.parse(data);
        if (Array.isArray(parsed)) return parsed.filter(x => typeof x === "string");
    } catch {}
    return [];
}

async function writeHostsRaw(hosts: string[]): Promise<void> {
    await ensureDir(CA_DIR);
    await fs.writeFile(CA_HOSTS_PATH, JSON.stringify(hosts, null, 2), { encoding: "utf-8", mode: 0o600 });
}

export async function existsCA(): Promise<boolean> {
    try {
        await fs.access(CA_CERT_PATH);
        await fs.access(CA_KEY_PATH);
        return true;
    } catch {
        return false;
    }
}

export async function readCACert(): Promise<string> {
    return await fs.readFile(CA_CERT_PATH, "utf-8");
}

export async function readCAKey(): Promise<string> {
    return await fs.readFile(CA_KEY_PATH, "utf-8");
}

export async function clearCA(): Promise<void> {
    try { await fs.unlink(CA_CERT_PATH); } catch {}
    try { await fs.unlink(CA_KEY_PATH); } catch {}
}

export async function initRootCA(config: {
    commonName: string;
    organization: string;
    organizationalUnit: string;
    country: string;
    state: string;
    locality: string;
    validityYears: number;
    keySize: number;
}): Promise<CAInfo> {
    await ensureDir(CA_DIR);
    const keys = forge.pki.rsa.generateKeyPair({ bits: config.keySize, workers: -1 });
    const cert = forge.pki.createCertificate();
    cert.publicKey = keys.publicKey;
    cert.serialNumber = genPositiveSerialHex(16);
    const now = new Date();
    cert.validity.notBefore = now;
    const end = new Date(now);
    end.setFullYear(now.getFullYear() + config.validityYears);
    cert.validity.notAfter = end;

    const attrs = [
        { name: "commonName", value: config.commonName },
        { name: "organizationName", value: config.organization },
        { name: "organizationalUnitName", value: config.organizationalUnit },
        { name: "countryName", value: config.country },
        { shortName: "ST", value: config.state },
        { name: "localityName", value: config.locality }
    ];
    cert.setSubject(attrs);
    cert.setIssuer(attrs);
    cert.setExtensions([
        { name: "basicConstraints", cA: true, pathLenConstraint: 0, critical: true },
        { name: "keyUsage", keyCertSign: true, cRLSign: true, digitalSignature: true, critical: true },
        { name: "subjectKeyIdentifier" }
    ]);
    const md = forge.md.sha256.create();
    cert.sign(keys.privateKey, md);
    const certPem = forge.pki.certificateToPem(cert);
    const keyPem = forge.pki.privateKeyToPem(keys.privateKey);
    await fs.writeFile(CA_CERT_PATH, certPem, { encoding: "utf-8", mode: 0o600 });
    await fs.writeFile(CA_KEY_PATH, keyPem, { encoding: "utf-8", mode: 0o600 });
    return await getCAInfo();
}

export async function getCAInfo(): Promise<CAInfo> {
    const pem = await readCACert();
    const x = new X509Certificate(pem);
    return {
        subject: x.subject,
        issuer: x.issuer,
        validFrom: x.validFrom,
        validTo: x.validTo,
        serialNumber: x.serialNumber
    };
}

export async function issueServerCert(hosts: string[], years: number): Promise<{ certPem: string; keyPem: string }> {
    if (!(await existsCA())) {
        throw new Error("Root CA not initialized");
    }
    if (!hosts.length) {
        throw new Error("No hosts configured");
    }
    const caCertPem = await readCACert();
    const caKeyPem = await readCAKey();
    const caCert = forge.pki.certificateFromPem(caCertPem);
    const caKey = forge.pki.privateKeyFromPem(caKeyPem);

    const keys = forge.pki.rsa.generateKeyPair({ bits: 2048, workers: -1 });
    const cert = forge.pki.createCertificate();
    cert.publicKey = keys.publicKey;
    cert.serialNumber = genPositiveSerialHex(16);
    const now = new Date();
    cert.validity.notBefore = now;
    const end = new Date(now);
    end.setFullYear(now.getFullYear() + years);
    cert.validity.notAfter = end;

    const cn = hosts[0] || "KCI-LNK Root Certificate";
    cert.setSubject([{ name: "commonName", value: cn }]);
    cert.setIssuer(caCert.subject.attributes);
    const altNames = hosts.map(h => (ipRegex.test(h) ? { type: 7, ip: h } : { type: 2, value: h }));
    cert.setExtensions([
        { name: "basicConstraints", cA: false },
        { name: "keyUsage", digitalSignature: true, keyEncipherment: true },
        { name: "extKeyUsage", serverAuth: true },
        { name: "subjectAltName", altNames }
    ]);
    const md = forge.md.sha256.create();
    cert.sign(caKey, md);
    const certPem = forge.pki.certificateToPem(cert);
    const keyPem = forge.pki.privateKeyToPem(keys.privateKey);
    return { certPem, keyPem };
}

const ipRegex = /^(?:(?:25[0-5]|2[0-4]\d|[01]?\d?\d)(?:\.|$)){4}$/;
function normalizeHost(value: string): string {
    return value.trim();
}

function genPositiveSerialHex(len: number): string {
    const buf = randomBytes(Math.max(1, len | 0));
    buf[0] = (buf[0]! & 0x7f);
    let allZero = true;
    for (let i = 0; i < buf.length; i++) {
        if (buf[i] !== 0) { allZero = false; break; }
    }
    if (allZero) buf[0] = 1;
    return Array.from(buf).map(b => b.toString(16).padStart(2, '0')).join('').toUpperCase();
}

export async function getHosts(): Promise<string[]> {
    return await readHostsRaw();
}

export async function addHost(value: string): Promise<string[]> {
    const v = normalizeHost(value);
    if (!v) return await readHostsRaw();
    const hosts = await readHostsRaw();
    if (!hosts.includes(v)) {
        hosts.push(v);
        await writeHostsRaw(hosts);
    }
    return hosts;
}

export async function removeHost(value: string): Promise<string[]> {
    const v = normalizeHost(value);
    const hosts = await readHostsRaw();
    const next = hosts.filter(h => h !== v);
    if (next.length !== hosts.length) {
        await writeHostsRaw(next);
    }
    return next;
}

export async function clearHosts(): Promise<void> {
    await writeHostsRaw([]);
}
