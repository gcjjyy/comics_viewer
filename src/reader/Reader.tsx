export function Reader({ onClose }: { libraryId: number; initialKey: string; onClose: () => void }) {
  return <button onClick={onClose}>← 책장</button>;
}
