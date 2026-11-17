"""Run only our own native smoke windows with a hard per-process timeout."""
import argparse
import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]

def current_run(output, before):
    runs=set(output.glob('ui-smoke-*'))-before
    if len(runs)!=1:
        raise RuntimeError(f'Expected one new native run, found {len(runs)}; old results cannot stand in.')
    return runs.pop()

def main():
    p=argparse.ArgumentParser();p.add_argument('--exe',type=Path,required=True);p.add_argument('--relocate',action='store_true');args=p.parse_args()
    exe=args.exe.resolve(); env=os.environ.copy()
    if args.relocate:
        folder=Path(tempfile.mkdtemp(prefix='Inconnu-package-check-'))
        relocated=folder/exe.name;shutil.copy2(exe,relocated);exe=relocated
        system_root=env.get('SystemRoot') or r'C:\Windows'
        env['PATH']=str(Path(system_root)/'System32')+';'+system_root
        env['SystemRoot']=system_root
    else: folder=ROOT
    results=[]
    results_path=ROOT/'artifacts'/('packaged-native-results.json' if args.relocate else 'native-results.json')
    results_path.parent.mkdir(parents=True, exist_ok=True)
    results_path.write_text('[]\n',encoding='utf-8')
    for scale in ['1.0','1.25']:
        output=ROOT/'artifacts'/('packaged-native' if args.relocate else 'native')/('scale-'+scale)
        output.mkdir(parents=True,exist_ok=True)
        env['GRAPEVINE_SMOKE_SCALE']=scale
        env['GRAPEVINE_DIAGNOSTICS_DIR']=str(output/'diagnostics')
        before=set(output.glob('ui-smoke-*'))
        try:
            completed=subprocess.run([str(exe),'--ui-smoke',str(output)],cwd=folder,env=env,capture_output=True,text=True,timeout=255)
        except subprocess.TimeoutExpired:
            runs=sorted(set(output.glob('ui-smoke-*'))-before,key=lambda x:x.stat().st_mtime)
            run=runs[-1] if runs else output
            phase=run/'native-smoke-phases.txt'
            failure={'ok':False,'scale':scale,'failure':'Native smoke process exceeded255seconds; subprocess.run killed only this test process.',
                     'run_directory':str(run),'last_phase':phase.read_text() if phase.is_file() else 'No phase breadcrumb available',
                     'executable':str(exe)}
            (run/'runner-timeout.json').write_text(json.dumps(failure,indent=2)+'\n',encoding='utf-8')
            results.append(failure);results_path.write_text(json.dumps(results,indent=2)+'\n',encoding='utf-8')
            raise RuntimeError('Native smoke timeout; retained '+str(run/'runner-timeout.json')) from None
        try:
            run=current_run(output,before)
        except RuntimeError:
            run=Path(tempfile.mkdtemp(prefix='runner-failure-',dir=output))
        (run/'runner-stdout.txt').write_text(completed.stdout,encoding='utf-8')
        (run/'runner-stderr.txt').write_text(completed.stderr,encoding='utf-8')
        paths=[run/'ui-smoke.json'] if (run/'ui-smoke.json').is_file() else []
        if not paths:
            phase=run/'native-smoke-phases.txt'
            failure={'ok':False,'scale':scale,'exit_code':completed.returncode,'failure':'Current native process produced no result; prior runs were excluded.',
                     'run_directory':str(run),'last_phase':phase.read_text() if phase.is_file() else 'No phase breadcrumb available','executable':str(exe)}
            (run/'runner-process-failure.json').write_text(json.dumps(failure,indent=2)+'\n',encoding='utf-8')
            results.append(failure);results_path.write_text(json.dumps(results,indent=2)+'\n',encoding='utf-8')
            raise RuntimeError('Native process failed; stdout/stderr and phase evidence retained in '+str(run))
        result=json.loads(paths[-1].read_text());result.update(executable=str(exe),scale=scale,exit_code=completed.returncode,minimal_path=args.relocate)
        results.append(result)
        results_path.write_text(json.dumps(results,indent=2)+'\n',encoding='utf-8')
        if completed.returncode!=0 or not result['ok'] or not all(result['checks'].values()):
            print(json.dumps({'failure':result.get('failure'),'scale':scale,'failed_checks':[key for key,value in result['checks'].items() if not value],'evidence':str(paths[-1])}))
            raise RuntimeError('Native verification failed; complete diagnostics retained in '+str(paths[-1]))
        assert Path(result['screenshot_path']).is_file(),result
        print('PASS native UI:',scale,paths[-1])
    (ROOT/'artifacts'/('packaged-native-results.json' if args.relocate else 'native-results.json')).write_text(json.dumps(results,indent=2)+'\n',encoding='utf-8')

if __name__ == "__main__":
    main()
