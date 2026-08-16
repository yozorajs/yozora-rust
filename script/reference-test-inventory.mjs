#!/usr/bin/env node

import crypto from 'node:crypto'
import fs from 'node:fs'
import path from 'node:path'
import process from 'node:process'
import { spawnSync } from 'node:child_process'
import { pathToFileURL } from 'node:url'

const args = process.argv.slice(2)

function readArg(name) {
  const index = args.indexOf(name)
  if (index < 0 || index + 1 >= args.length) return null
  return args[index + 1]
}

const referenceRootArg = readArg('--reference-root') ?? process.env.YOZORA_REFERENCE_ROOT
const outputPath = readArg('--output')
const shouldCheck = args.includes('--check')

if (referenceRootArg == null || referenceRootArg.length === 0) {
  throw new Error('Pass --reference-root or set YOZORA_REFERENCE_ROOT')
}
const referenceRoot = path.resolve(referenceRootArg)
if (!fs.existsSync(referenceRoot)) throw new Error(`Reference root does not exist: ${referenceRoot}`)
if (shouldCheck && outputPath == null) {
  throw new Error('--check requires --output')
}

const typescriptPath = path.join(referenceRoot, 'node_modules/typescript/lib/typescript.js')
if (!fs.existsSync(typescriptPath)) {
  throw new Error(`TypeScript is unavailable at ${typescriptPath}; do not install it implicitly`)
}
const ts = (await import(pathToFileURL(typescriptPath).href)).default

function collectFiles(root, predicate) {
  const result = []
  const stack = [root]
  while (stack.length > 0) {
    const current = stack.pop()
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const target = path.join(current, entry.name)
      if (entry.isDirectory()) stack.push(target)
      else if (entry.isFile() && predicate(target)) result.push(target)
    }
  }
  return result.sort()
}

function relativePath(filepath) {
  return path.relative(referenceRoot, filepath).replaceAll('\\', '/')
}

function hash(value) {
  return crypto.createHash('sha256').update(value).digest('hex')
}

function fnv1a64(value) {
  let result = 0xcbf29ce484222325n
  for (const byte of Buffer.from(value)) {
    result ^= BigInt(byte)
    result = BigInt.asUintN(64, result * 0x100000001b3n)
  }
  return result.toString(16).padStart(16, '0')
}

function sourceText(source, node) {
  return node == null ? '<missing>' : node.getText(source).replace(/\s+/g, ' ').trim()
}

function unwrapExpression(node) {
  let current = node
  while (
    current != null &&
    (ts.isParenthesizedExpression(current) ||
      ts.isAsExpression(current) ||
      ts.isSatisfiesExpression(current) ||
      ts.isTypeAssertionExpression(current) ||
      ts.isNonNullExpression(current))
  ) {
    current = current.expression
  }
  return current
}

function literalValue(source, node, constants, environment) {
  const current = unwrapExpression(node)
  if (current == null) return undefined
  if (ts.isStringLiteralLike(current) || ts.isNumericLiteral(current)) return current.text
  if (current.kind === ts.SyntaxKind.TrueKeyword) return true
  if (current.kind === ts.SyntaxKind.FalseKeyword) return false
  if (current.kind === ts.SyntaxKind.NullKeyword) return null
  if (ts.isIdentifier(current)) {
    if (environment.has(current.text)) return environment.get(current.text)
    const initializer = constants.get(current.text)
    return initializer == null
      ? undefined
      : literalValue(source, initializer, constants, environment)
  }
  if (
    ts.isCallExpression(current) &&
    ts.isPropertyAccessExpression(current.expression) &&
    ts.isIdentifier(current.expression.expression) &&
    current.expression.expression.text === 'JSON' &&
    current.expression.name.text === 'stringify'
  ) {
    const value = literalValue(source, current.arguments[0], constants, environment)
    return value === undefined ? undefined : JSON.stringify(value)
  }
  return undefined
}

function renderTitle(source, node, constants, environment) {
  const current = unwrapExpression(node)
  if (current == null) return '<missing>'
  if (ts.isStringLiteralLike(current)) return current.text
  if (!ts.isTemplateExpression(current)) return sourceText(source, current)

  let result = current.head.text
  for (const span of current.templateSpans) {
    const value = literalValue(source, span.expression, constants, environment)
    result += value === undefined ? `\${${sourceText(source, span.expression)}}` : String(value)
    result += span.literal.text
  }
  return result
}

function collectConstants(source) {
  const constants = new Map()
  function visit(node) {
    if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && node.initializer != null) {
      constants.set(node.name.text, node.initializer)
    }
    ts.forEachChild(node, visit)
  }
  visit(source)
  return constants
}

function resolveArray(source, node, constants, environment) {
  const current = unwrapExpression(node)
  if (current == null) return null
  if (ts.isIdentifier(current)) {
    if (environment.has(current.text)) {
      const value = environment.get(current.text)
      return Array.isArray(value) ? value : null
    }
    const initializer = constants.get(current.text)
    return initializer == null ? null : resolveArray(source, initializer, constants, environment)
  }
  if (ts.isArrayLiteralExpression(current)) {
    return current.elements.map(element => ({
      display: sourceText(source, element),
      value: literalValue(source, element, constants, environment),
      node: element,
    }))
  }
  if (
    ts.isCallExpression(current) &&
    current.arguments.length === 0 &&
    ts.isPropertyAccessExpression(current.expression) &&
    current.expression.name.text === 'entries'
  ) {
    const values = resolveArray(source, current.expression.expression, constants, environment)
    return values?.map((value, index) => ({
      display: `[${index}, ${value.display}]`,
      value: [index, value.value],
      node: value.node,
    })) ?? null
  }
  return null
}

function bindName(name, value, environment) {
  const next = new Map(environment)
  if (ts.isIdentifier(name)) {
    next.set(name.text, value)
    return next
  }
  if (ts.isArrayBindingPattern(name) && Array.isArray(value)) {
    for (let index = 0; index < name.elements.length; index += 1) {
      const element = name.elements[index]
      if (ts.isBindingElement(element) && ts.isIdentifier(element.name)) {
        next.set(element.name.text, value[index])
      }
    }
    return next
  }
  return next
}

function describeCall(node) {
  return (
    ts.isCallExpression(node) &&
    ts.isIdentifier(node.expression) &&
    node.expression.text === 'describe'
  )
}

function testCall(node) {
  if (!ts.isCallExpression(node)) return null
  if (ts.isIdentifier(node.expression) && ['test', 'it'].includes(node.expression.text)) {
    return { kind: node.expression.text, data: null }
  }
  if (
    ts.isPropertyAccessExpression(node.expression) &&
    ts.isIdentifier(node.expression.expression) &&
    ['test', 'it'].includes(node.expression.expression.text) &&
    ['skip', 'only', 'todo'].includes(node.expression.name.text)
  ) {
    return { kind: `${node.expression.expression.text}.${node.expression.name.text}`, data: null }
  }
  if (!ts.isCallExpression(node.expression)) return null
  const registration = node.expression
  if (
    !ts.isPropertyAccessExpression(registration.expression) ||
    !ts.isIdentifier(registration.expression.expression) ||
    !['test', 'it'].includes(registration.expression.expression.text) ||
    registration.expression.name.text !== 'each'
  ) {
    return null
  }
  return { kind: `${registration.expression.expression.text}.each`, data: registration.arguments[0] }
}

function numericLoopValues(statement) {
  if (
    statement.initializer == null ||
    !ts.isVariableDeclarationList(statement.initializer) ||
    statement.initializer.declarations.length !== 1
  ) {
    return null
  }
  const declaration = statement.initializer.declarations[0]
  if (!ts.isIdentifier(declaration.name) || declaration.initializer == null) return null
  const start = Number(literalValue(null, declaration.initializer, new Map(), new Map()))
  if (!Number.isFinite(start) || statement.condition == null || statement.incrementor == null) {
    return null
  }
  if (!ts.isBinaryExpression(statement.condition)) return null
  if (!ts.isIdentifier(statement.condition.left) || statement.condition.left.text !== declaration.name.text) {
    return null
  }
  const end = Number(literalValue(null, statement.condition.right, new Map(), new Map()))
  if (!Number.isFinite(end)) return null
  const operator = statement.condition.operatorToken.kind
  const incrementor = statement.incrementor
  const increments =
    (ts.isPrefixUnaryExpression(incrementor) || ts.isPostfixUnaryExpression(incrementor)) &&
    incrementor.operator === ts.SyntaxKind.PlusPlusToken &&
    ts.isIdentifier(incrementor.operand) &&
    incrementor.operand.text === declaration.name.text
  if (!increments) return null

  const limit = operator === ts.SyntaxKind.LessThanEqualsToken ? end : end - 1
  if (operator !== ts.SyntaxKind.LessThanEqualsToken && operator !== ts.SyntaxKind.LessThanToken) {
    return null
  }
  const values = []
  for (let value = start; value <= limit; value += 1) values.push(value)
  return { name: declaration.name, values }
}

function collectDirectTests(filepath) {
  const content = fs.readFileSync(filepath, 'utf8')
  const source = ts.createSourceFile(filepath, content, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS)
  const constants = collectConstants(source)
  const sourcePath = relativePath(filepath)
  const tests = []
  const scanners = []

  function visitStatements(statements, context) {
    for (const statement of statements) visitStatement(statement, context)
  }

  function visitStatement(statement, context) {
    if (ts.isBlock(statement)) {
      visitStatements(statement.statements, context)
      return
    }
    if (ts.isForOfStatement(statement)) {
      const values = resolveArray(source, statement.expression, constants, context.environment)
      const declaration = ts.isVariableDeclarationList(statement.initializer)
        ? statement.initializer.declarations[0]
        : null
      if (values == null || declaration == null) {
        throw new Error(`Unresolved registration for-of loop at ${sourcePath}`)
      }
      values.forEach((entry, index) => {
        visitStatement(statement.statement, {
          ...context,
          environment: bindName(declaration.name, entry.value, context.environment),
          registrationPath: [...context.registrationPath, `for-of-${index}:${entry.display}`],
        })
      })
      return
    }
    if (ts.isForStatement(statement)) {
      const loop = numericLoopValues(statement)
      if (loop == null) throw new Error(`Unresolved registration for-loop at ${sourcePath}`)
      loop.values.forEach(value => {
        visitStatement(statement.statement, {
          ...context,
          environment: bindName(loop.name, value, context.environment),
          registrationPath: [...context.registrationPath, `for-${loop.name.text}=${value}`],
        })
      })
      return
    }
    if (!ts.isExpressionStatement(statement)) return

    const scannerRegistration = sourceText(source, statement.expression)
    function collectRunTest(node) {
      if (
        ts.isCallExpression(node) &&
        ts.isPropertyAccessExpression(node.expression) &&
        node.expression.name.text === 'runTest'
      ) {
        const location = source.getLineAndCharacterOfPosition(node.getStart(source))
        scanners.push({
          id: `${sourcePath}:${location.line + 1}`,
          source: sourcePath,
          line: location.line + 1,
          expression: scannerRegistration,
        })
      }
      ts.forEachChild(node, collectRunTest)
    }
    collectRunTest(statement.expression)

    if (!ts.isCallExpression(statement.expression)) return
    const call = statement.expression
    if (describeCall(call)) {
      const callback = call.arguments[1]
      if (
        callback != null &&
        (ts.isArrowFunction(callback) || ts.isFunctionExpression(callback)) &&
        ts.isBlock(callback.body)
      ) {
        visitStatements(callback.body.statements, {
          ...context,
          describes: [
            ...context.describes,
            renderTitle(source, call.arguments[0], constants, context.environment),
          ],
        })
      }
      return
    }

    const registration = testCall(call)
    if (registration != null) {
      const location = source.getLineAndCharacterOfPosition(call.getStart(source))
      const line = location.line + 1
      const statementId = `${sourcePath}:${line}`
      const registrationSuffix = context.registrationPath.join('/')
      const registrationId = registrationSuffix
        ? `${statementId}[${registrationSuffix}]`
        : statementId
      const rows =
        registration.data == null
          ? [{ display: null }]
          : resolveArray(source, registration.data, constants, context.environment)
      if (rows == null) throw new Error(`Unresolved test.each data at ${statementId}`)
      const title = [
        ...context.describes,
        renderTitle(source, call.arguments[0], constants, context.environment),
      ].join(' > ')
      rows.forEach((row, index) => {
        tests.push({
          id: `${registrationId}::${index}`,
          statement_id: statementId,
          source: sourcePath,
          line,
          kind: registration.kind,
          title,
          parameters: row.display,
        })
      })
      return
    }

  }

  visitStatements(source.statements, {
    describes: [],
    environment: new Map(),
    registrationPath: [],
  })

  const discoveredTestStatements = new Set()
  const discoveredScanners = new Set()
  function discoverRegistrations(node) {
    if (ts.isCallExpression(node)) {
      const location = source.getLineAndCharacterOfPosition(node.getStart(source))
      const registrationId = `${sourcePath}:${location.line + 1}`
      if (testCall(node) != null) discoveredTestStatements.add(registrationId)
      if (
        ts.isPropertyAccessExpression(node.expression) &&
        node.expression.name.text === 'runTest'
      ) {
        discoveredScanners.add(registrationId)
      }
    }
    ts.forEachChild(node, discoverRegistrations)
  }
  discoverRegistrations(source)

  const inventoriedTestStatements = new Set(tests.map(test => test.statement_id))
  const inventoriedScanners = new Set(scanners.map(scanner => scanner.id))
  for (const registrationId of discoveredTestStatements) {
    if (!inventoriedTestStatements.has(registrationId)) {
      throw new Error(`Unsupported test registration context at ${registrationId}`)
    }
  }
  for (const registrationId of inventoriedTestStatements) {
    if (!discoveredTestStatements.has(registrationId)) {
      throw new Error(`Inventoried test registration was not found at ${registrationId}`)
    }
  }
  for (const registrationId of discoveredScanners) {
    if (!inventoriedScanners.has(registrationId)) {
      throw new Error(`Unsupported fixture scanner context at ${registrationId}`)
    }
  }
  for (const registrationId of inventoriedScanners) {
    if (!discoveredScanners.has(registrationId)) {
      throw new Error(`Inventoried fixture scanner was not found at ${registrationId}`)
    }
  }

  return {
    source: sourcePath,
    sha256: hash(content),
    tests,
    scanners,
  }
}

function slugify(value, fallbackIndex) {
  const slug = String(value ?? '')
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
  return slug || `case-${fallbackIndex + 1}`
}

function collectFixtureCases() {
  const fixtureRoot = path.join(referenceRoot, 'fixtures')
  const files = collectFiles(fixtureRoot, filepath => filepath.endsWith('.json'))
  const cases = []
  const fixtureFiles = []
  const hashedFiles = []
  for (const filepath of files) {
    const content = fs.readFileSync(filepath, 'utf8')
    const relative = relativePath(filepath).replace(/^fixtures\//, '')
    fixtureFiles.push({ source: relative, fnv1a64: fnv1a64(content) })
    hashedFiles.push(`${relative}\0${content}`)
    const document = JSON.parse(content)
    if (!Array.isArray(document.cases)) continue
    document.cases.forEach((item, index) => {
      const description = typeof item.description === 'string' ? item.description : null
      cases.push({
        id: `${relative}::${index}::${slugify(description, index)}`,
        source: relative,
        index,
        description,
        has_parse_answer: Object.hasOwn(item, 'parseAnswer'),
        has_markup_answer: Object.hasOwn(item, 'markupAnswer'),
      })
    })
  }
  return {
    sha256: hash(hashedFiles.sort().join('\0')),
    files: fixtureFiles,
    cases,
  }
}

const testFiles = collectFiles(referenceRoot, filepath =>
  /(?:^|\/)(?:packages|tokenizers)\/.*\/(?:__test__\/.*|[^/]+)(?:\.spec|\.test)\.ts$/.test(
    filepath.replaceAll('\\', '/'),
  ),
)
const testInventory = testFiles.map(collectDirectTests)
const fixtureInventory = collectFixtureCases()
const gitResult = spawnSync('git', ['-C', referenceRoot, 'rev-parse', 'HEAD'], {
  encoding: 'utf8',
})
if (gitResult.status !== 0) {
  throw new Error(`Failed to resolve reference commit: ${gitResult.stderr.trim()}`)
}
const referenceCommit = gitResult.stdout.trim()

const inventory = {
  schema_version: 2,
  reference_commit: referenceCommit,
  test_files: testInventory.map(item => ({
    source: item.source,
    sha256: item.sha256,
  })),
  direct_tests: testInventory.flatMap(item => item.tests),
  fixture_scanners: testInventory.flatMap(item => item.scanners),
  fixture_tree_sha256: fixtureInventory.sha256,
  fixture_files: fixtureInventory.files,
  fixture_cases: fixtureInventory.cases,
}
const serialized = `${JSON.stringify(inventory, null, 2)}\n`

if (shouldCheck) {
  const actual = fs.readFileSync(path.resolve(outputPath), 'utf8')
  if (actual !== serialized) {
    throw new Error(`Reference test inventory differs from ${outputPath}`)
  }
} else if (outputPath != null) {
  fs.mkdirSync(path.dirname(path.resolve(outputPath)), { recursive: true })
  fs.writeFileSync(path.resolve(outputPath), serialized)
} else {
  process.stdout.write(serialized)
}
