/**
 * Mock 模式测试文件
 * 用于验证各个 API 端点在 Mock 模式下正常返回数据
 * 
 * 使用方式：
 * 1. 启动 Mock 模式：bun dev:mock
 * 2. 在浏览器控制台运行此文件中的测试函数
 */

import { api } from "./api";

// 设置日志颜色
const log = {
  success: (msg: string) => console.log(`%c✓ ${msg}`, "color: green; font-weight: bold"),
  error: (msg: string) => console.error(`%c✗ ${msg}`, "color: red; font-weight: bold"),
  info: (msg: string) => console.log(`%c➜ ${msg}`, "color: blue; font-weight: bold"),
};

/**
 * 测试查询功能
 */
export async function testQuery() {
  log.info("Testing query.execute...");
  try {
    const result = await api.query.execute(1, "SELECT * FROM users LIMIT 10");
    log.success("query.execute returned data with " + result.rowCount + " rows");
    console.log("Result:", result);
  } catch (error) {
    log.error("query.execute failed: " + (error as Error).message);
  }
}

/**
 * 测试元数据功能
 */
export async function testMetadata() {
  log.info("Testing metadata.listTables...");
  try {
    const tables = await api.metadata.listTables(1);
    log.success("metadata.listTables returned " + tables.length + " tables");
    console.log("Tables:", tables);
  } catch (error) {
    log.error("metadata.listTables failed: " + (error as Error).message);
  }

  log.info("Testing metadata.getTableStructure...");
  try {
    const structure = await api.metadata.getTableStructure(1, "public", "users");
    log.success("metadata.getTableStructure returned " + structure.columns.length + " columns");
    console.log("Structure:", structure);
  } catch (error) {
    log.error("metadata.getTableStructure failed: " + (error as Error).message);
  }

  log.info("Testing metadata.getTableMetadata...");
  try {
    const metadata = await api.metadata.getTableMetadata(1, undefined, "public", "users");
    log.success("metadata.getTableMetadata returned " + metadata.columns.length + " columns");
    console.log("Metadata:", metadata);
  } catch (error) {
    log.error("metadata.getTableMetadata failed: " + (error as Error).message);
  }

  log.info("Testing metadata.getSchemaOverview...");
  try {
    const overview = await api.metadata.getSchemaOverview(1);
    log.success("metadata.getSchemaOverview returned " + overview.tables.length + " tables");
    console.log("Overview:", overview);
  } catch (error) {
    log.error("metadata.getSchemaOverview failed: " + (error as Error).message);
  }
}

/**
 * 测试表数据功能
 */
export async function testTableData() {
  log.info("Testing tableData.get...");
  try {
    const data = await api.tableData.get({
      id: 1,
      schema: "public",
      table: "users",
      page: 1,
      limit: 10,
    });
    log.success("tableData.get returned " + data.data.length + " rows");
    console.log("Data:", data);
  } catch (error) {
    log.error("tableData.get failed: " + (error as Error).message);
  }
}

/**
 * 测试连接功能
 */
export async function testConnections() {
  log.info("Testing connections.list...");
  try {
    const connections = await api.connections.list();
    log.success("connections.list returned " + connections.length + " connections");
    console.log("Connections:", connections);
  } catch (error) {
    log.error("connections.list failed: " + (error as Error).message);
  }
}

/**
 * 运行所有测试
 */
export async function runAllTests() {
  console.log("%c========== Mock API 测试开始 ==========", "color: purple; font-weight: bold; font-size: 14px");
  
  await testQuery();
  console.log("");
  
  await testMetadata();
  console.log("");
  
  await testTableData();
  console.log("");
  
  await testConnections();
  
  console.log("%c========== Mock API 测试结束 ==========", "color: purple; font-weight: bold; font-size: 14px");
}

// 导出测试函数，方便在浏览器控制台调用
if (typeof window !== "undefined") {
  (window as any).testMockAPI = {
    runAllTests,
    testQuery,
    testMetadata,
    testTableData,
    testConnections,
  };
}
