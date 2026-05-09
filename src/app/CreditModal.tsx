import { useEffect, useState } from 'react';
import { getVersion } from '@tauri-apps/api/app';
import { open } from '@tauri-apps/plugin-shell';
import { StellaIcon, stellaIconNames } from '../shared/components/Icons';
import shared from '../shared/styles/shared.module.css';
import styles from './CreditModal.module.css';
import avatarSrc from '../assets/avatar.jpg';
import logoDarkSrc from '../assets/logo-dark.png';

const LINKS = [
  { label: 'lit.link', icon: stellaIconNames.sparkle, url: 'https://lit.link/planet_vrc' },
  { label: 'X (Twitter)', icon: stellaIconNames.bell, url: 'https://x.com/planet_vrc' },
  { label: 'BOOTH', icon: stellaIconNames.rocket, url: 'https://cosmo-arts-store.booth.pm/' },
] as const;

export function CreditButton() {
  const [isOpen, setIsOpen] = useState(false);

  return (
    <>
      <button
        className={styles.trigger}
        onClick={() => { setIsOpen(true); }}
        aria-label="クレジット"
      >
        <StellaIcon name={stellaIconNames.info} style={{ width: '18px', height: '18px', fill: 'currentColor' }} />
      </button>

      {isOpen && <CreditModal onClose={() => { setIsOpen(false); }} />}
    </>
  );
}

function CreditModal({ onClose }: { onClose: () => void }) {
  const [version, setVersion] = useState('');

  useEffect(() => {
    void getVersion().then(setVersion);
  }, []);

  const handleLink = (url: string) => {
    void open(url);
  };

  return (
    <div className={shared.modalOverlay} onClick={onClose}>
      <div
        className={`${shared.modalContent} ${styles.modal}`}
        onClick={(e) => { e.stopPropagation(); }}
      >
        <div className={styles.banner}>
          <div className={styles.bannerGlow} />
          <img src={logoDarkSrc} alt="STELLA RECORD" className={styles.bannerLogo} />
          {version && <span className={styles.bannerVersion}>v{version}</span>}
        </div>

        <div className={styles.body}>
          <div className={styles.profile}>
            <img src={avatarSrc} alt="ぷらねっと" className={styles.avatar} />
            <div className={styles.profileText}>
              <p className={styles.brandName}>CosmoArtsStore</p>
              <p className={styles.creatorName}>by ぷらねっと</p>
            </div>
          </div>

          <div className={styles.divider} />

          <div className={styles.links}>
            {LINKS.map((link) => (
              <button
                key={link.label}
                className={styles.linkItem}
                onClick={() => { handleLink(link.url); }}
              >
                <span className={styles.linkIcon}>
                  <StellaIcon name={link.icon} style={{ width: '14px', height: '14px', fill: 'currentColor' }} />
                </span>
                <span className={styles.linkLabel}>{link.label}</span>
                <span className={styles.linkArrow}>&#8250;</span>
              </button>
            ))}
          </div>

          <div className={styles.divider} />

          <div className={styles.attribution}>
            <p className={styles.attributionTitle}>使用素材</p>
            <p className={styles.attributionItem}>
              SVG Icons —{' '}
              <button className={styles.attributionLink} onClick={() => { handleLink('https://pictogrammers.com/library/mdi/'); }}>
                Material Design Icons (Pictogrammers)
              </button>
              {' '}/ Apache License 2.0
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}
