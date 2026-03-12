import * as os from 'os';
import * as path from 'path';
import * as fs from 'fs';

export class AppDirManager {
  private static instance: AppDirManager;
  
  private readonly dataDirPath: string;

  private constructor() {
    this.dataDirPath = this.initializeDirectory();
  }

  public static getInstance(): AppDirManager {
    if (!AppDirManager.instance) {
      AppDirManager.instance = new AppDirManager();
    }
    return AppDirManager.instance;
  }

  private initializeDirectory(): string {
    const explicitDataDir = process.env.FN_KNOCK_DATA_DIR?.trim();
    if (explicitDataDir) {
      if (!fs.existsSync(explicitDataDir)) {
        fs.mkdirSync(explicitDataDir, { recursive: true });
        console.log(`[AppDirManager] 使用环境变量目录并已创建: ${explicitDataDir}`);
      }
      return explicitDataDir;
    }

    const platform = os.platform();
    const homeDir = os.homedir();
    const appName = 'fn-knock';
    let targetPath = '';

    if (platform === 'darwin') {
      targetPath = path.join(homeDir, 'Library', 'Application Support', appName);
    } else if (platform === 'linux') {
      targetPath = path.join(homeDir, '.local', 'share', appName);
    } else {
      throw new Error(`Unsupported operating system: ${platform}`);
    }

    if (!fs.existsSync(targetPath)) {
      fs.mkdirSync(targetPath, { recursive: true });
      console.log(`[AppDirManager] 目录已创建: ${targetPath}`);
    }
    return targetPath;
  }

  public getDataPath(): string {
    return this.dataDirPath;
  }
}

const dirManager = AppDirManager.getInstance();
export const dataPath = dirManager.getDataPath();
