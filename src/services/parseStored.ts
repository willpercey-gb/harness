import type { ToolEvent } from '@/types/chat.types'

export interface ParsedAssistant {
  content: string
  reasoning: string
  toolEvents: ToolEvent[]
}

const REASONING_TAGS = ['thinking', 'reasoning'] as const

/**
 * Mirror of crates/harness-chat/src/pipeline/xml_unwrap.rs for assistant
 * messages reloaded from the database. Pulls <thinking>/<reasoning> blocks
 * into `reasoning`, captures <tool_use>/<tool_result> markers into
 * structured events, and strips <response> wrappers.
 *
 * Anything not matching a known tag is passed through verbatim into
 * `content` (so the original prose survives).
 */
export function parseAssistantContent(raw: string): ParsedAssistant {
  let content = raw

  // Reasoning blocks (multiple supported, concatenated with newlines).
  const reasoningParts: string[] = []
  for (const tag of REASONING_TAGS) {
    const re = new RegExp(`<${tag}>([\\s\\S]*?)</${tag}>`, 'g')
    content = content.replace(re, (_match, body: string) => {
      reasoningParts.push(body.trim())
      return ''
    })
  }

  // Tool events. The original streaming pipeline emits these in order, so
  // we walk the raw string in document order to preserve the same sequence.
  const toolEvents: ToolEvent[] = []
  const toolPattern =
    /<tool_use\b([^>]*)>(?:<\/tool_use>)?|<tool_result\b([^>]*)>(?:<\/tool_result>)?/g
  let m: RegExpExecArray | null
  while ((m = toolPattern.exec(raw)) !== null) {
    const useAttrs = m[1]
    const resultAttrs = m[2]
    if (useAttrs !== undefined) {
      toolEvents.push({
        kind: 'tool_use',
        name: attr(useAttrs, 'name') ?? '',
        id: attr(useAttrs, 'id') ?? '',
      })
    } else if (resultAttrs !== undefined) {
      const status = attr(resultAttrs, 'status')
      toolEvents.push({
        kind: 'tool_result',
        name: attr(resultAttrs, 'name') ?? '',
        id: attr(resultAttrs, 'id') ?? '',
        status: status === 'success' ? 'success' : 'error',
      })
    }
  }
  // Strip the tool tags from the displayed content.
  content = content.replace(toolPattern, '')

  // Strip <response>…</response> wrappers (keep their inner text).
  content = content.replace(/<response>([\s\S]*?)<\/response>/g, '$1')
  content = content.replace(/<\/?response>/g, '')

  return {
    content: content.trim(),
    reasoning: reasoningParts.join('\n\n'),
    toolEvents,
  }
}

function attr(attrs: string, key: string): string | undefined {
  const re = new RegExp(`${key}="([^"]*)"`)
  const m = re.exec(attrs)
  return m ? m[1] : undefined
}
