import { useEffect, useState } from "react";
import { MessageSquareText, Save, X } from "lucide-react";
import {
  getReplyStyleSettings,
  saveReplyStyleSettings,
} from "../services/tauriApi";

const REPLY_STYLE_EXAMPLES = [
  {
    label: "详细解释",
    text: "请回答得更详细、有层次。先直接给出结论，再分 2-3 点解释原因。如果涉及技术方案，要说明取舍、风险和适用场景。保持口语化，像我在会议里自然说出来。",
  },
  {
    label: "面试回答",
    text: "请按面试回答的方式组织：先给明确结论，再结合项目经验解释原因，最后补一句权衡或总结。回答要专业、有逻辑，但不要太书面化。",
  },
  {
    label: "技术答辩",
    text: "请用技术答辩风格回答。优先解释设计动机、核心机制、边界情况和风险控制。如果问题很宽泛，先给整体判断，再展开关键点。",
  },
] as const;

type ReplyStylePanelProps = {
  onClose: () => void;
  closeTitle?: string;
  className?: string;
};

export function ReplyStylePanel({
  onClose,
  closeTitle = "关闭回复风格设置",
  className = "modalPanel replyStylePanel",
}: ReplyStylePanelProps) {
  const [replyStylePrompt, setReplyStylePrompt] = useState("");
  const [status, setStatus] = useState("");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    void getReplyStyleSettings()
      .then((settings) => {
        setReplyStylePrompt(settings.userPrompt ?? "");
      })
      .catch(() => {
        setStatus("加载回复风格设置失败");
      });
  }, []);

  async function saveReplyStyle() {
    setSaving(true);
    setStatus("");
    try {
      const saved = await saveReplyStyleSettings({
        userPrompt: replyStylePrompt,
      });
      setReplyStylePrompt(saved.userPrompt);
      setStatus("回复风格已保存");
    } catch (error) {
      setStatus(
        error instanceof Error ? error.message : "保存回复风格失败",
      );
    } finally {
      setSaving(false);
    }
  }

  return (
    <section
      aria-labelledby="reply-style-title"
      className={className}
      role="dialog"
    >
      <div className="modalHeader">
        <div>
          <h2 id="reply-style-title">回复风格</h2>
          <div className="configStatus">
            自定义 LLM 回复的语气、长度和结构
          </div>
        </div>
        <button type="button" onClick={onClose} title={closeTitle}>
          <X size={16} />
        </button>
      </div>

      <div className="replyStylePanelBody">
        <div className="replyStyleIntro">
          <div className="replyStyleIntroIcon" aria-hidden="true">
            <MessageSquareText size={16} />
          </div>
          <p>
            这段提示词只控制回复风格，不会覆盖系统安全规则。留空则使用默认风格。
          </p>
        </div>

        <label className="replyStyleField">
          <span>回复风格提示词</span>
          <textarea
            className="replyStyleTextarea"
            value={replyStylePrompt}
            onChange={(event) => setReplyStylePrompt(event.target.value)}
            placeholder="例如：请回答得更详细、有层次。先给结论，再分 2-3 点说明原因。保持像真人在会议里说话。"
            maxLength={2000}
            rows={5}
          />
        </label>

        <div className="replyStyleHint">
          每次 LLM 请求会携带当前保存的设置；已开始的生成不受影响。
        </div>

        <div className="replyStyleExamples">
          <span className="replyStyleExamplesLabel">快捷示例</span>
          <div className="replyStyleExampleButtons">
            {REPLY_STYLE_EXAMPLES.map((example) => (
              <button
                key={example.label}
                type="button"
                className="replyStyleExampleButton"
                onClick={() => setReplyStylePrompt(example.text)}
              >
                {example.label}
              </button>
            ))}
          </div>
        </div>
      </div>

      <div className="modalFooter replyStylePanelFooter">
        {status ? <div className="configStatus">{status}</div> : <div />}
        <div className="replyStyleActionButtons">
          <button
            type="button"
            className="secondaryButton"
            onClick={() => setReplyStylePrompt("")}
          >
            恢复默认
          </button>
          <button
            type="button"
            className="saveButton"
            disabled={saving}
            onClick={() => void saveReplyStyle()}
          >
            <Save size={15} aria-hidden="true" />
            保存风格
          </button>
        </div>
      </div>
    </section>
  );
}
