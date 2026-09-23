#!/usr/bin/env python3
"""Plot existing frozen v5 evidence; does not run a benchmark or estimate new CIs."""
import argparse,hashlib,json
from pathlib import Path
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--input',type=Path,default=Path(__file__).with_name('main-summary.json'))
p.add_argument('--out',type=Path,default=Path(__file__).with_name('main-benefits'))
a=p.parse_args();data=json.loads(a.input.read_text())
assert data['quality']['trials']==data['quality']['valid_trials']==108
assert data['acceptance']['passed'] is True
labels={'bootstrap':'Bootstrap','session_api':'Session API','session_hit':'Valid session','grant_hit':'Valid grant','auto_ip_hit':'Auto-IP hit','auto_ip_miss':'Auto-IP miss','challenge':'Challenge (guard)','session_miss':'Invalid session (guard)','grant_miss':'Invalid grant (guard)'}
rows=data['groups'];assert len(rows)==9 and {r['scenario'] for r in rows}==set(labels)
plt.rcParams.update({'font.family':'DejaVu Sans','font.size':10,'axes.spines.top':False,'axes.spines.right':False,'axes.spines.left':False,'svg.fonttype':'none','svg.hashsalt':'auth-v5-20260923'})
fig,axes=plt.subplots(1,3,figsize=(13.6,7.0),sharey=True,gridspec_kw={'width_ratios':[3.1,2.6,2.0]})
fig.subplots_adjust(left=.19,right=.97,top=.73,bottom=.26,wspace=.22)
settings=[('throughput_change_median','throughput_change_bootstrap_95','Throughput change (%)',(-12,335)),('p99_change_median','p99_change_bootstrap_95','P99 change (%)',(-76,18)),('rss',None,'Combined peak RSS (%)',(-6.4,6.8))]
for ax,(key,ci_key,title,limits) in zip(axes,settings):
    ax.axvline(0,color='#59636d',lw=.8)
    if key=='rss':ax.axvline(5,color='#b36330',lw=1,ls='--',label='5% gate')
    for y,row in enumerate(rows):
        c=row['comparison'];assert c['six_valid_pairs']
        value=100*(c['peak_rss_change_median']['combined'] if key=='rss' else c[key])
        color='#147a9c' if row['gate']=='improvement' else '#72808b'
        if ci_key:
            low,high=[100*x for x in c[ci_key]]
            ax.errorbar(value,y,xerr=[[value-low],[high-value]],fmt='o',ms=5,color=color,elinewidth=1.5,capsize=3)
            text_x=high+(limits[1]-limits[0])*.018
        else:
            ax.plot(value,y,'o',ms=5,color=color);text_x=value+.27
        ax.text(text_x,y,f'{value:+.1f}%',va='center',fontsize=9,color=color)
    ax.set_title(title,loc='left',fontsize=11,pad=17,fontweight='bold')
    ax.set_xlim(*limits);ax.set_ylim(8.65,-.65);ax.tick_params(axis='y',length=0)
    ax.grid(axis='x',alpha=.12);ax.set_axisbelow(True)
axes[0].set_yticks(range(9),[labels[r['scenario']] for r in rows])
axes[0].set_xlabel('Higher is better',loc='left',fontsize=9,labelpad=13)
axes[1].set_xlabel('Lower is better',loc='left',fontsize=9,labelpad=13)
axes[2].set_xlabel('Lower; dashed line = +5% gate',loc='left',fontsize=9,labelpad=13)
fig.text(.045,.94,'Authentication performance: qualified v5 effects',fontsize=20,fontweight='bold',color='#18252f')
fig.text(.045,.892,'Original Rust d4f8805f / Go 92d4c0c  →  v5 Rust 5fddf896 / Go 4d15fa3',fontsize=11,color='#455864')
fig.text(.045,.855,'108 valid trials · six independent AB/BA pairs per route · c16 · cache TTL 0 · 20 s warmup / 60 s load',fontsize=10,color='#455864')
fig.text(.045,.14,'Points: median of six within-pair changes. Whiskers: the existing paired-bootstrap 95% intervals for throughput and P99.',fontsize=9,color='#455864')
fig.text(.045,.105,'RSS is the sum of each service’s sampled peak, not necessarily simultaneous peaks. No RSS interval is estimated in this figure.',fontsize=9,color='#455864')
fig.text(.045,.07,'These v5 results exclude the later Go 66998225 invalidation repair; its independent cost and validation are reported separately.',fontsize=9,color='#455864')
a.out.parent.mkdir(parents=True,exist_ok=True)
fig.savefig(str(a.out)+'.png',dpi=170,facecolor='white')
fig.savefig(str(a.out)+'.svg',facecolor='white',metadata={'Date':None})
(a.out.with_suffix('.json')).write_text(json.dumps({'input':a.input.name,'input_sha256':hashlib.sha256(a.input.read_bytes()).hexdigest(),'generator_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'matplotlib':matplotlib.__version__,'source_scope':'v5 only; no audit6 results or compounded ratios','new_intervals_estimated':False},indent=2)+'\n')
print(str(a.out)+'.png')
