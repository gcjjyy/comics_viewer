import type { Direction } from "../api";

interface Props {
  direction: Direction;
  onClose: () => void;
}

/** Reader shortcuts. Arrow meanings follow the book's reading direction. */
export function ShortcutHelp({ direction, onClose }: Props) {
  const rtl = direction === "rtl";
  const rows: [string, string][] = [
    ["←", rtl ? "다음 쪽" : "이전 쪽"],
    ["→", rtl ? "이전 쪽" : "다음 쪽"],
    ["Space / Shift+Space", "다음 쪽 / 이전 쪽"],
    ["화면 왼쪽·오른쪽 클릭", "그쪽으로 넘기기"],
    ["마우스 휠", "다음 쪽 / 이전 쪽"],
    ["아래 진행 막대 클릭", "그 위치로 이동"],
    ["D", "이 책의 읽는 방향 바꾸기"],
    ["S", "한 쪽 / 두 쪽 보기"],
    ["A", "펼침 짝을 한 쪽 밀기"],
    ["F", "전체 화면"],
    ["?", "단축키 보기"],
    ["Esc", "책장으로"],
  ];
  return (
    <div className="modal-back" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-head">
          <b>단축키</b>
          <button className="icon-btn" onClick={onClose} aria-label="닫기">✕</button>
        </div>
        <div className="modal-body">
          <table className="keys">
            <tbody>
              {rows.map(([k, v]) => (
                <tr key={k}>
                  <td><kbd>{k}</kbd></td>
                  <td>{v}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
