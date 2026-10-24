import { BindingSettings } from '../shared/BindingSettings'
import { Button } from '../shared/Button'
export function SetupStep({ onNext }: { onNext: () => void }) {
  return <div><h1>Set your shortcuts</h1><BindingSettings /><Button onClick={onNext}>Continue</Button></div>
}
